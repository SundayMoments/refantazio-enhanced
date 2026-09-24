// Executes the actual shipping HLSL on D3D11 with synthetic, known inputs.
#define NOMINMAX
#include <windows.h>
#include <d3d11.h>
#include <d3d11sdklayers.h>
#include <d3dcompiler.h>
#include <wrl/client.h>
#include <array>
#include <cmath>
#include <cstring>
#include <filesystem>
#include <functional>
#include <fstream>
#include <iostream>
#include <limits>
#include <map>
#include <stdexcept>
#include <string>
#include <vector>
#include "../upstream/Source/Games/Metaphor ReFantazio/temporal_quality.h"

using Microsoft::WRL::ComPtr;
constexpr UINT W = 13, H = 9; // Exercises partial compute thread groups.
void check(HRESULT hr) { if (FAILED(hr)) throw std::runtime_error("D3D HRESULT " + std::to_string(hr)); }
void require(bool ok, const char* message) { if (!ok) throw std::runtime_error(message); }
void close(float actual, float expected, float tolerance = 0.0001f)
{
    if (!std::isfinite(actual) || std::abs(actual - expected) > tolerance)
        throw std::runtime_error("expected " + std::to_string(expected) + ", got " + std::to_string(actual));
}

struct Texture
{
    ComPtr<ID3D11Texture2D> texture;
    ComPtr<ID3D11ShaderResourceView> srv;
    ComPtr<ID3D11UnorderedAccessView> uav;
    UINT channels;
};
struct Shader
{
    ComPtr<ID3D11ComputeShader> shader;
    ComPtr<ID3DBlob> blob;
    ComPtr<ID3D11ShaderReflection> reflection;
};
class Includes : public ID3DInclude
{
    std::filesystem::path root;
    std::map<const void*, std::filesystem::path> parents;
public:
    explicit Includes(std::filesystem::path root_path) : root(std::move(root_path)) {}
    HRESULT __stdcall Open(D3D_INCLUDE_TYPE, LPCSTR name, LPCVOID parent, LPCVOID* data, UINT* bytes) override
    {
        auto base = parents.contains(parent) ? parents.at(parent).parent_path() : root;
        auto file = base / name;
        std::ifstream input(file, std::ios::binary | std::ios::ate);
        if (!input) return E_FAIL;
        const auto size = input.tellg();
        auto contents = new char[size_t(size)];
        input.seekg(0); input.read(contents, size);
        *data = contents; *bytes = UINT(size);
        parents[contents] = file;
        return S_OK;
    }
    HRESULT __stdcall Close(LPCVOID data) override
    {
        parents.erase(data); delete[] static_cast<const char*>(data); return S_OK;
    }
};
class GPU
{
public:
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    ComPtr<ID3D11InfoQueue> messages;
    ComPtr<ID3D11SamplerState> point, linear;
    GPU()
    {
        D3D_FEATURE_LEVEL level;
        HRESULT hr = D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, D3D11_CREATE_DEVICE_DEBUG,
            nullptr, 0, D3D11_SDK_VERSION, &device, &level, &context);
        if (FAILED(hr)) check(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, 0,
            nullptr, 0, D3D11_SDK_VERSION, &device, &level, &context));
        device.As(&messages);
        ComPtr<IDXGIDevice> dxgi;
        check(device.As(&dxgi));
        ComPtr<IDXGIAdapter> adapter;
        check(dxgi->GetAdapter(&adapter));
        DXGI_ADAPTER_DESC desc{};
        check(adapter->GetDesc(&desc));
        std::wcout << L"GPU: " << desc.Description << L"; debug layer: " << (messages ? L"yes" : L"no") << L"\n";
        D3D11_SAMPLER_DESC sd{};
        sd.Filter = D3D11_FILTER_MIN_MAG_MIP_POINT;
        sd.AddressU = sd.AddressV = sd.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
        sd.MaxLOD = D3D11_FLOAT32_MAX;
        check(device->CreateSamplerState(&sd, &point));
        sd.Filter = D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT;
        check(device->CreateSamplerState(&sd, &linear));
    }
    Shader compile(const std::filesystem::path& path, bool history = false, bool fallback = false)
    {
        Shader result;
        ComPtr<ID3DBlob> error;
        D3D_SHADER_MACRO macros[] = {{"HAS_PREVIOUS_FRAME", "1"}, {nullptr, nullptr}};
        D3D_SHADER_MACRO fallback_macros[] = {{"SR_FALLBACK", "1"}, {nullptr, nullptr}};
        Includes includes(std::filesystem::absolute(path).parent_path());
        HRESULT hr = D3DCompileFromFile(path.c_str(), fallback ? fallback_macros : history ? macros : nullptr, &includes,
            "main", "cs_5_0", D3DCOMPILE_OPTIMIZATION_LEVEL3 | D3DCOMPILE_ENABLE_STRICTNESS, 0, &result.blob, &error);
        if (FAILED(hr) && error) std::cerr << static_cast<const char*>(error->GetBufferPointer()) << '\n';
        check(hr);
        check(device->CreateComputeShader(result.blob->GetBufferPointer(), result.blob->GetBufferSize(), nullptr, &result.shader));
        check(D3DReflect(result.blob->GetBufferPointer(), result.blob->GetBufferSize(), IID_PPV_ARGS(&result.reflection)));
        return result;
    }
    Texture texture(UINT channels, const std::vector<float>& data, bool output = false, UINT width = W, UINT height = H)
    {
        Texture result; result.channels = channels;
        D3D11_TEXTURE2D_DESC td{};
        td.Width = width; td.Height = height; td.ArraySize = td.MipLevels = 1;
        td.Format = channels == 1 ? DXGI_FORMAT_R32_FLOAT : channels == 2 ? DXGI_FORMAT_R32G32_FLOAT : DXGI_FORMAT_R32G32B32A32_FLOAT;
        td.SampleDesc.Count = 1;
        td.BindFlags = D3D11_BIND_SHADER_RESOURCE | (output ? D3D11_BIND_UNORDERED_ACCESS : 0);
        D3D11_SUBRESOURCE_DATA initial{data.data(), width * channels * sizeof(float), 0};
        check(device->CreateTexture2D(&td, data.empty() ? nullptr : &initial, &result.texture));
        check(device->CreateShaderResourceView(result.texture.Get(), nullptr, &result.srv));
        if (output) check(device->CreateUnorderedAccessView(result.texture.Get(), nullptr, &result.uav));
        return result;
    }
    ComPtr<ID3D11Buffer> buffer(const void* data, UINT size)
    {
        ComPtr<ID3D11Buffer> result;
        D3D11_BUFFER_DESC bd{};
        bd.ByteWidth = size; bd.BindFlags = D3D11_BIND_CONSTANT_BUFFER;
        D3D11_SUBRESOURCE_DATA initial{data, 0, 0};
        check(device->CreateBuffer(&bd, &initial, &result));
        return result;
    }
    ComPtr<ID3D11Buffer> settings(Shader& shader, UINT sr_type = 1, UINT render_width = W, UINT render_height = H)
    {
        auto cb = shader.reflection->GetConstantBufferByName("LumaSettings");
        D3D11_SHADER_BUFFER_DESC bd{};
        check(cb->GetDesc(&bd));
        std::vector<unsigned char> bytes(bd.Size);
        auto var = cb->GetVariableByName("LumaSettings");
        D3D11_SHADER_VARIABLE_DESC vd{}; check(var->GetDesc(&vd));
        auto type = var->GetType();
        D3D11_SHADER_TYPE_DESC td{};
        check(type->GetMemberTypeByName("SRType")->GetDesc(&td));
        std::memcpy(bytes.data() + vd.StartOffset + td.Offset, &sr_type, sizeof(sr_type));
        check(type->GetMemberTypeByName("GameSettings")->GetDesc(&td));
        const UINT game_offset = vd.StartOffset + td.Offset;
        auto game = type->GetMemberTypeByName("GameSettings");
        check(game->GetMemberTypeByName("RenderRes")->GetDesc(&td));
        const float size[] = {float(render_width), float(render_height)};
        std::memcpy(bytes.data() + game_offset + td.Offset, size, sizeof(size));
        check(game->GetMemberTypeByName("OutputRes")->GetDesc(&td));
        const float output_size[] = {float(W), float(H)};
        std::memcpy(bytes.data() + game_offset + td.Offset, output_size, sizeof(output_size));
        for (const char* name : {"InvRenderRes", "InvOutputRes"})
        {
            const bool render = std::strcmp(name, "InvRenderRes") == 0;
            const float inv_size[] = {1.f/(render ? render_width : W), 1.f/(render ? render_height : H)};
            check(game->GetMemberTypeByName(name)->GetDesc(&td));
            std::memcpy(bytes.data() + game_offset + td.Offset, inv_size, sizeof(inv_size));
        }
        const float scale = float(render_width)/W;
        check(game->GetMemberTypeByName("RenderScale")->GetDesc(&td));
        std::memcpy(bytes.data() + game_offset + td.Offset, &scale, sizeof(scale));
        return buffer(bytes.data(), bd.Size);
    }
    std::vector<float> run(Shader& shader, const std::vector<ID3D11ShaderResourceView*>& srvs,
        Texture& out, ID3D11Buffer* cb = nullptr, ID3D11Buffer* global = nullptr)
    {
        context->ClearState();
        if (messages) messages->ClearStoredMessages();
        context->CSSetShader(shader.shader.Get(), nullptr, 0);
        context->CSSetShaderResources(0, UINT(srvs.size()), srvs.data());
        auto uav = out.uav.Get(); context->CSSetUnorderedAccessViews(0, 1, &uav, nullptr);
        if (cb) context->CSSetConstantBuffers(0, 1, &cb);
        if (global) context->CSSetConstantBuffers(13, 1, &global);
        ID3D11SamplerState* samplers[] = {linear.Get(), point.Get()};
        context->CSSetSamplers(7, 2, samplers);
        context->CSSetSamplers(0, 1, samplers);
        context->Dispatch((W + 7) / 8, (H + 7) / 8, 1);
        context->ClearState();
        D3D11_TEXTURE2D_DESC desc{}; out.texture->GetDesc(&desc);
        desc.Usage = D3D11_USAGE_STAGING; desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ; desc.BindFlags = 0;
        ComPtr<ID3D11Texture2D> read;
        check(device->CreateTexture2D(&desc, nullptr, &read));
        context->CopyResource(read.Get(), out.texture.Get());
        D3D11_MAPPED_SUBRESOURCE mapped{};
        check(context->Map(read.Get(), 0, D3D11_MAP_READ, 0, &mapped));
        std::vector<float> values(W * H * out.channels);
        for (UINT y = 0; y < H; ++y)
            std::memcpy(values.data() + y * W * out.channels,
                static_cast<unsigned char*>(mapped.pData) + y * mapped.RowPitch, W * out.channels * sizeof(float));
        context->Unmap(read.Get(), 0);
        if (messages)
        {
            for (UINT64 i = 0; i < messages->GetNumStoredMessages(); ++i)
            {
                SIZE_T size = 0; messages->GetMessage(i, nullptr, &size);
                std::vector<char> storage(size);
                auto message = reinterpret_cast<D3D11_MESSAGE*>(storage.data());
                check(messages->GetMessage(i, message, &size));
                if (message->Severity <= D3D11_MESSAGE_SEVERITY_WARNING)
                    throw std::runtime_error(message->pDescription);
            }
        }
        return values;
    }
};

struct Matrix
{
    float m00 = 1, m01 = 0, m02 = 0, m03 = 0;
    float m10 = 0, m11 = 1, m12 = 0, m13 = 0;
    float m20 = 0, m21 = 0, m22 = 1, m23 = 0;
    float m30 = 0, m31 = 0, m32 = 0, m33 = 1;
};
struct alignas(16) DepthConstants
{
    float screen[4] = {float(W), float(H), 1.f/W, 1.f/H};
    int variance = 0;
    float variance_scale = 1;
    float velocity_scale[2] = {-0.5f, 0.5f};
    float jitter[2] = {};
    float padding[2] = {};
};

int wmain(int argc, wchar_t** argv)
{
    if (argc != 2) { std::cerr << "Usage: temporal_gpu <Metaphor shader directory>\n"; return 2; }
    int passed = 0, failed = 0;
    auto test = [&](const char* name, const std::function<void()>& fn) {
        try { fn(); ++passed; std::cout << "PASS " << name << '\n'; }
        catch (const std::exception& e) { ++failed; std::cout << "FAIL " << name << ": " << e.what() << '\n'; }
    };
    try
    {
        GPU gpu;
        const std::filesystem::path root = argv[1];
        auto motion = gpu.compile(root / "Luma_PrepareMotionVector.hlsl");
        auto depth = gpu.compile(root / "Luma_TemporalAADepth.hlsl", true);
        auto depth_first = gpu.compile(root / "Luma_TemporalAADepth.hlsl");
        auto mask = gpu.compile(root / "Luma_CreateBiasMask.hlsl");
        auto merge = gpu.compile(root / "Luma_CopyDsrResult.hlsl");
        auto fallback = gpu.compile(root / "Luma_CopyDsrResult.hlsl", false, true);
        auto settings = gpu.settings(motion);
        auto zero_mv = gpu.texture(2, std::vector<float>(W*H*2));
        auto sky_depth = gpu.texture(1, std::vector<float>(W*H, 1.f));
        auto output_mv = gpu.texture(2, {}, true);
        test("SR merge keeps resolved RGB and pixel-aligned bloom alpha", [&] {
            std::vector<float> original(W*H*4), resolved(W*H*4, 0.75f);
            for (UINT i=0;i<W*H;++i) original[i*4+3] = float(i%2);
            auto source=gpu.texture(4,original), result=gpu.texture(4,resolved), output=gpu.texture(4,{},true);
            auto cb=gpu.settings(merge);
            auto values=gpu.run(merge,{result.srv.Get(),source.srv.Get()},output,nullptr,cb.Get());
            for(UINT i=0;i<W*H;++i) {
                for(UINT c=0;c<3;++c) close(values[i*4+c],0.75f);
                close(values[i*4+3],original[i*4+3]);
            }
        });
        test("failed SR uses current color even when resolved texture is stale", [&] {
            std::vector<float> original(W*H*4), stale(W*H*4, 999.f);
            for(UINT i=0;i<W*H*4;++i) original[i]=float(i%7)/7.f;
            auto source=gpu.texture(4,original), result=gpu.texture(4,stale), output=gpu.texture(4,{},true);
            auto cb=gpu.settings(fallback);
            auto values=gpu.run(fallback,{result.srv.Get(),source.srv.Get()},output,nullptr,cb.Get());
            for(UINT i=0;i<W*H*4;++i) close(values[i],original[i]);
        });
        test("failed upscaling covers the output using current source pixels", [&] {
            constexpr UINT rw=7, rh=5;
            std::vector<float> original(rw*rh*4,1.f);
            for(UINT y=0;y<rh;++y) for(UINT x=0;x<rw;++x) {
                original[(y*rw+x)*4]=float(x)/rw;
                original[(y*rw+x)*4+1]=float(y)/rh;
                original[(y*rw+x)*4+2]=0.25f;
            }
            auto source=gpu.texture(4,original,false,rw,rh), output=gpu.texture(4,{},true);
            auto cb=gpu.settings(fallback,1,rw,rh);
            auto values=gpu.run(fallback,{nullptr,source.srv.Get()},output,nullptr,cb.Get());
            for(UINT y=0;y<H;++y) for(UINT x=0;x<W;++x) {
                close(values[(y*W+x)*4],std::clamp((x+0.5f)*rw/W-0.5f,0.f,float(rw-1))/rw,0.001f);
                close(values[(y*W+x)*4+1],std::clamp((y+0.5f)*rh/H-0.5f,0.f,float(rh-1))/rh,0.001f);
                close(values[(y*W+x)*4+2],0.25f); close(values[(y*W+x)*4+3],1.f);
            }
        });
        test("sky rotation has correct direction and pixel centers", [&] {
            Matrix m; const float angle = 0.2f;
            m.m00 = m.m11 = std::cos(angle); m.m01 = -std::sin(angle); m.m10 = std::sin(angle);
            auto cb = gpu.buffer(&m, sizeof(m));
            auto values = gpu.run(motion, {zero_mv.srv.Get(), sky_depth.srv.Get()}, output_mv, cb.Get(), settings.Get());
            for (UINT y=0; y<H; ++y) for (UINT x=0; x<W; ++x)
            {
                float u=(x+0.5f)/W, v=(y+0.5f)/H, nx=u*2-1, ny=1-v*2;
                float px=m.m00*nx+m.m01*ny, py=m.m10*nx+m.m11*ny;
                close(values[(y*W+x)*2], ((px+1)*0.5f-u)*W);
                close(values[(y*W+x)*2+1], ((1-py)*0.5f-v)*H);
            }
        });
        test("stationary sky stays stationary", [&] {
            Matrix m; auto cb=gpu.buffer(&m,sizeof(m));
            auto values=gpu.run(motion,{zero_mv.srv.Get(),sky_depth.srv.Get()},output_mv,cb.Get(),settings.Get());
            for(float v:values) close(v,0.f);
        });
        test("invalid sky history is finite and outside viewport", [&] {
            Matrix m; m.m33=0; auto cb=gpu.buffer(&m,sizeof(m));
            auto values=gpu.run(motion,{zero_mv.srv.Get(),sky_depth.srv.Get()},output_mv,cb.Get(),settings.Get());
            close(values[0],2.f*W); close(values[1],2.f*H);
        });
        test("geometry motion remains in previous-minus-current pixel units", [&] {
            std::vector<float> values(W*H*2);
            for(size_t i=0;i<values.size();i+=2) { values[i]=0.2f; values[i+1]=-0.3f; }
            auto mv=gpu.texture(2,values), z=gpu.texture(1,std::vector<float>(W*H,0.5f));
            Matrix m; auto cb=gpu.buffer(&m,sizeof(m));
            auto result=gpu.run(motion,{mv.srv.Get(),z.srv.Get()},output_mv,cb.Get(),settings.Get());
            close(result[0],-0.1f*W); close(result[1],-0.15f*H);
        });
        auto output_depth=gpu.texture(1,{},true);
        auto flat=gpu.texture(1,std::vector<float>(W*H,0.5f));
        test("flat depth history remains finite", [&] {
            DepthConstants constants; auto cb=gpu.buffer(&constants,sizeof(constants));
            auto values=gpu.run(depth,{flat.srv.Get(),flat.srv.Get(),zero_mv.srv.Get()},output_depth,cb.Get());
            for(float v:values) close(v,0.5f);
        });
        test("first depth frame does not sample history", [&] {
            DepthConstants constants; auto cb=gpu.buffer(&constants,sizeof(constants));
            auto values=gpu.run(depth_first,{flat.srv.Get()},output_depth,cb.Get());
            for(float v:values) close(v,0.5f);
        });
        test("off-screen depth history is rejected", [&] {
            std::vector<float> z(W*H,0.5f); const UINT center=4*W+6;
            z[center-1]=0.4f; z[center+1]=0.6f;
            auto current=gpu.texture(1,z), previous=gpu.texture(1,std::vector<float>(W*H,0.45f));
            auto mv=gpu.texture(2,std::vector<float>(W*H*2,4.f));
            DepthConstants constants; auto cb=gpu.buffer(&constants,sizeof(constants));
            auto values=gpu.run(depth,{current.srv.Get(),previous.srv.Get(),mv.srv.Get()},output_depth,cb.Get());
            close(values[center],0.5f);
        });
        test("depth jitter reprojects to the previous sample location", [&] {
            std::vector<float> z(W*H,0.5f), prev(W*H,0.45f); const UINT center=4*W+6;
            z[center-1]=0.4f; z[center+1]=0.6f; prev[center+1]=0.5f;
            auto current=gpu.texture(1,z), previous=gpu.texture(1,prev);
            DepthConstants constants; constants.jitter[0]=0.75f/W;
            auto cb=gpu.buffer(&constants,sizeof(constants));
            auto values=gpu.run(depth,{current.srv.Get(),previous.srv.Get(),zero_mv.srv.Get()},output_depth,cb.Get());
            close(values[center],0.5f);
        });
        test("DLSS particle mask is binary and ignores faint coverage", [&] {
            std::vector<float> data(W*H*4,0.f);
            for(UINT i=0;i<W*H;++i) data[i*4+3]=(i%3==0)?1.f:(i%3==1)?0.9f:0.1f;
            auto input=gpu.texture(4,data), output=gpu.texture(1,{},true);
            auto global=gpu.settings(mask,1);
            auto values=gpu.run(mask,{input.srv.Get()},output,nullptr,global.Get());
            for(UINT i=0;i<W*H;++i) close(values[i],i%3==2?1.f:0.f);
        });
        test("FSR retains continuous particle reactivity", [&] {
            std::vector<float> data(W*H*4,0.7f);
            auto input=gpu.texture(4,data), output=gpu.texture(1,{},true);
            auto global=gpu.settings(mask,2);
            auto values=gpu.run(mask,{input.srv.Get()},output,nullptr,global.Get());
            for(float v:values) close(v,0.3f);
        });
        test("camera history handles steady motion, cuts and zoom discontinuities", [&] {
            Matrix identity, smooth_rotation, cut, zoom;
            smooth_rotation.m00=smooth_rotation.m11=std::cos(0.1f); smooth_rotation.m01=-std::sin(0.1f); smooth_rotation.m10=std::sin(0.1f);
            cut.m00=cut.m11=0.f; cut.m01=-1.f; cut.m10=1.f;
            zoom.m00=zoom.m11=1.5f;
            require(!MetaphorTemporal::CameraDiscontinuity(identity,identity,identity,identity),"stationary camera reset");
            require(!MetaphorTemporal::CameraDiscontinuity(smooth_rotation,identity,identity,identity),"smooth camera reset");
            require(MetaphorTemporal::CameraDiscontinuity(cut,identity,identity,identity),"missed camera cut");
            require(MetaphorTemporal::CameraDiscontinuity(identity,identity,zoom,identity),"missed projection cut");
            Matrix invalid; invalid.m00=std::numeric_limits<float>::quiet_NaN();
            require(MetaphorTemporal::CameraDiscontinuity(invalid,identity,identity,identity),"missed invalid camera");
        });
        test("texture policy preserves native mip and supports upstream comparison", [&] {
            close(MetaphorTemporal::TextureBias(2160,2160,0),0);
            close(MetaphorTemporal::TextureBias(1080,2160,0),-1);
            close(MetaphorTemporal::TextureBias(2160,2160,-1),-1);
            close(MetaphorTemporal::TextureBias(0,2160,0),0);
        });
        test("history jitter follows the actual projection handedness", [&] {
            close(MetaphorTemporal::HistoryJitterPixels(0.25f,-0.25f,1,1),0.5f);
            close(MetaphorTemporal::HistoryJitterPixels(0.25f,-0.25f,-1,-1),-0.5f);
            close(MetaphorTemporal::HistoryJitterPixels(0.25f,-0.25f,0,0),0.f);
        });
    }
    catch(const std::exception& e) { std::cerr << "HARNESS ERROR: " << e.what() << '\n'; return 2; }
    std::cout << passed << " passed, " << failed << " failed\n";
    return failed ? 1 : 0;
}

