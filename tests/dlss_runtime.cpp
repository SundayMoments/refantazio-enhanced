// Exercises the shipping backend with the local NVIDIA runtime, in release mode.
// Wrappers observe API calls and inject failures; successful work runs on the GPU.
#define NOMINMAX
#include <windows.h>
#include <d3d11.h>
#include <wrl/client.h>
#include <DirectXPackedVector.h>
#include <iostream>
#include <limits>
#include <vector>
#include <stdexcept>
#include "nvsdk_ngx_helpers.h"

namespace Audit {
    unsigned released = 0, destroyed = 0, shutdowns = 0, evaluations = 0;
    bool fail_create = false, fail_draw = false;
    bool cleanup_ok = true;
    NVSDK_NGX_D3D11_DLSS_Eval_Params last{};
    NVSDK_NGX_Result Release(NVSDK_NGX_Handle* feature) {
        ++released; auto result = NVSDK_NGX_D3D11_ReleaseFeature(feature);
        cleanup_ok &= NVSDK_NGX_SUCCEED(result); return result;
    }
    NVSDK_NGX_Result Destroy(NVSDK_NGX_Parameter* parameters) {
        ++destroyed; auto result = NVSDK_NGX_D3D11_DestroyParameters(parameters);
        cleanup_ok &= NVSDK_NGX_SUCCEED(result); return result;
    }
    NVSDK_NGX_Result Shutdown(ID3D11Device* device) {
        ++shutdowns; auto result = NVSDK_NGX_D3D11_Shutdown1(device);
        cleanup_ok &= NVSDK_NGX_SUCCEED(result); return result;
    }
    NVSDK_NGX_Result Create(ID3D11DeviceContext* context, NVSDK_NGX_Handle** feature,
        NVSDK_NGX_Parameter* parameters, NVSDK_NGX_DLSS_Create_Params* settings) {
        if (fail_create) return NVSDK_NGX_Result_FAIL_UnableToInitializeFeature;
        return NGX_D3D11_CREATE_DLSS_EXT(context, feature, parameters, settings);
    }
    NVSDK_NGX_Result Evaluate(ID3D11DeviceContext* context, NVSDK_NGX_Handle* feature,
        NVSDK_NGX_Parameter* parameters, NVSDK_NGX_D3D11_DLSS_Eval_Params* data) {
        ++evaluations; last = *data;
        if (fail_draw) return NVSDK_NGX_Result_FAIL_PlatformError;
        auto result = NGX_D3D11_EVALUATE_DLSS_EXT(context, feature, parameters, data);
        if (NVSDK_NGX_FAILED(result)) std::cerr << "NGX evaluate failed: " << std::hex << result << std::dec << '\n';
        return result;
    }
}
#define NVSDK_NGX_D3D11_ReleaseFeature Audit::Release
#define NVSDK_NGX_D3D11_DestroyParameters Audit::Destroy
#define NVSDK_NGX_D3D11_Shutdown1 Audit::Shutdown
#define NGX_D3D11_CREATE_DLSS_EXT Audit::Create
#define NGX_D3D11_EVALUATE_DLSS_EXT Audit::Evaluate
#include "../upstream/Source/Core/dlss/DLSS.cpp"
#undef NVSDK_NGX_D3D11_ReleaseFeature
#undef NVSDK_NGX_D3D11_DestroyParameters
#undef NVSDK_NGX_D3D11_Shutdown1
#undef NGX_D3D11_CREATE_DLSS_EXT
#undef NGX_D3D11_EVALUATE_DLSS_EXT

using Microsoft::WRL::ComPtr;
void require(bool condition, const char* message) { if (!condition) throw std::runtime_error(message); }
void check(HRESULT hr) { require(SUCCEEDED(hr), "D3D11 operation failed"); }

int main()
{
    static_assert(
#ifdef NDEBUG
        true
#else
        false
#endif
        , "Run with NDEBUG: assert must not be responsible for NGX cleanup");
    NGX::DLSS backend;
    SR::InstanceData* instance = nullptr;
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    int passed = 0;
    auto pass = [&](const char* name) { ++passed; std::cout << "PASS " << name << std::endl; };
    try {
        check(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, 0, nullptr, 0,
            D3D11_SDK_VERSION, &device, nullptr, &context));
        require(backend.Init(instance, device.Get()), "local NVIDIA DLSS runtime unavailable");
        pass("initialize actual local NGX runtime");
        constexpr unsigned size = 128;
        auto texture = [&](DXGI_FORMAT format, unsigned pixel_bytes, const void* data, bool output = false, unsigned width = 128) {
            D3D11_TEXTURE2D_DESC desc{};
            desc.Width = desc.Height = width; desc.MipLevels = desc.ArraySize = desc.SampleDesc.Count = 1;
            desc.Format = format;
            desc.BindFlags = D3D11_BIND_SHADER_RESOURCE | (output ? D3D11_BIND_UNORDERED_ACCESS : 0);
            D3D11_SUBRESOURCE_DATA initial{data, width * pixel_bytes, 0};
            ComPtr<ID3D11Texture2D> result;
            check(device->CreateTexture2D(&desc, data ? &initial : nullptr, &result));
            return result;
        };
        std::vector<unsigned short> colors(size*size*4, 0x3800); // 0.5 linear
        std::vector<float> depths(size*size, 0.5f), motion(size*size*2, 0.f);
        std::vector<unsigned short> masks(size*size, 0);
        unsigned short one = 0x3c00;
        auto color = texture(DXGI_FORMAT_R16G16B16A16_FLOAT, 8, colors.data());
        auto output = texture(DXGI_FORMAT_R16G16B16A16_FLOAT, 8, nullptr, true);
        auto depth = texture(DXGI_FORMAT_R32_FLOAT, 4, depths.data());
        auto mv = texture(DXGI_FORMAT_R32G32_FLOAT, 8, motion.data());
        auto mask = texture(DXGI_FORMAT_R16_FLOAT, 2, masks.data());
        auto exposure = texture(DXGI_FORMAT_R16_FLOAT, 2, &one, false, 1);
        SR::SettingsData settings;
        settings.render_width = settings.render_height = settings.output_width = settings.output_height = size;
        settings.render_preset = 11; // K, matching the installed default
        require(backend.UpdateSettings(instance, context.Get(), settings), "create DLAA feature failed");
        SR::SuperResolutionImpl::DrawData draw;
        draw.source_color = color.Get(); draw.output_color = output.Get(); draw.depth_buffer = depth.Get();
        draw.motion_vectors = mv.Get(); draw.current_color_bias = mask.Get(); draw.exposure = exposure.Get();
        draw.render_width = draw.render_height = size;
        draw.jitter_x = 0.25f; draw.jitter_y = -0.125f;
        require(backend.Draw(instance, context.Get(), draw), "DLAA evaluation failed");
        require(Audit::last.InReset == 1, "first evaluation did not reset history");
        require(Audit::last.pInBiasCurrentColorMask == mask.Get() && Audit::last.pInExposureTexture == exposure.Get(), "lost optional inputs");
        require(Audit::last.InMVScaleX == 1.f && Audit::last.InMVScaleY == 1.f &&
            Audit::last.InJitterOffsetX == draw.jitter_x && Audit::last.InJitterOffsetY == draw.jitter_y, "wrong motion/jitter units");
        pass("first DLAA frame resets and forwards mask, exposure, jitter and motion units");
        require(backend.Draw(instance, context.Get(), draw) && Audit::last.InReset == 0, "steady frame reset unexpectedly");
        pass("steady frame reuses temporal history");
        context->ClearState();
        D3D11_TEXTURE2D_DESC read_desc{}; output->GetDesc(&read_desc);
        read_desc.BindFlags = 0; read_desc.Usage = D3D11_USAGE_STAGING; read_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        ComPtr<ID3D11Texture2D> read;
        check(device->CreateTexture2D(&read_desc, nullptr, &read));
        context->CopyResource(read.Get(), output.Get());
        D3D11_MAPPED_SUBRESOURCE mapped{}; check(context->Map(read.Get(), 0, D3D11_MAP_READ, 0, &mapped));
        bool valid = true;
        for (unsigned y=0; y<size; ++y) {
            auto row = reinterpret_cast<const unsigned short*>(static_cast<const char*>(mapped.pData) + y*mapped.RowPitch);
            for (unsigned x=0; x<size; ++x) for(unsigned c=0;c<3;++c) {
                float value = DirectX::PackedVector::XMConvertHalfToFloat(row[x*4+c]);
                valid &= std::isfinite(value) && std::abs(value - 0.5f) < 0.1f;
            }
        }
        context->Unmap(read.Get(), 0);
        require(valid, "DLAA output did not preserve constant linear RGB");
        pass("GPU readback preserves finite linear color");
        Audit::fail_draw = true;
        require(!backend.Draw(instance, context.Get(), draw), "injected evaluation failure ignored");
        Audit::fail_draw = false;
        require(backend.Draw(instance, context.Get(), draw) && Audit::last.InReset == 1, "recovery used broken history");
        pass("failed evaluation returns false and recovery resets history");
        auto invalid = draw; invalid.jitter_x = std::numeric_limits<float>::quiet_NaN();
        auto calls = Audit::evaluations;
        require(!backend.Draw(instance, context.Get(), invalid) && calls == Audit::evaluations, "non-finite jitter reached NGX");
        invalid = draw; invalid.source_color = nullptr;
        require(!backend.Draw(instance, context.Get(), invalid) && calls == Audit::evaluations, "null input reached NGX");
        invalid = draw; invalid.render_width = size + 1;
        require(!backend.Draw(instance, context.Get(), invalid) && calls == Audit::evaluations, "oversized subrect reached NGX");
        require(!backend.Draw(nullptr, context.Get(), draw), "null backend not rejected");
        pass("invalid inputs are rejected before NGX evaluation");
        auto zero = settings; zero.render_width = 0;
        require(!backend.UpdateSettings(instance, context.Get(), zero), "zero dimensions accepted");
        pass("zero-sized feature rejected");
        context->ClearState(); context->Flush();
        Audit::fail_create = true; settings.render_preset = 0;
        require(!backend.UpdateSettings(instance, context.Get(), settings), "injected creation failure ignored");
        require(!backend.Draw(instance, context.Get(), draw) && calls == Audit::evaluations, "failed feature reached evaluation");
        Audit::fail_create = false;
        require(backend.UpdateSettings(instance, context.Get(), settings), "feature recreation did not recover");
        require(backend.Draw(instance, context.Get(), draw) && Audit::last.InReset == 1, "recreated feature retained history");
        pass("creation failure is safe and recreation resets history");
        // Also exercise the actual upscaling mode, not only native DLAA.
        auto upscaled = texture(DXGI_FORMAT_R16G16B16A16_FLOAT, 8, nullptr, true, size*2);
        settings.output_width = settings.output_height = size*2;
        context->ClearState(); context->Flush();
        require(backend.UpdateSettings(instance, context.Get(), settings), "upscaling feature failed");
        draw.output_color = upscaled.Get();
        require(backend.Draw(instance, context.Get(), draw) && Audit::last.InReset == 1, "upscaling evaluation failed");
        pass("actual 2x DLSS upscaling evaluates successfully");
        context->ClearState(); context->Flush();
        unsigned released = Audit::released, destroyed = Audit::destroyed;
        backend.Deinit(instance);
        require(!instance && Audit::cleanup_ok && Audit::released == released+1 && Audit::destroyed == destroyed+2 && Audit::shutdowns == 1,
            "release build did not release feature/parameters and shut down NGX");
        pass("NDEBUG cleanup releases feature, runtime/capability parameters and NGX");
        std::cout << passed << " passed, 0 failed\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "FAIL " << error.what() << '\n';
        if (context) { context->ClearState(); context->Flush(); }
        backend.Deinit(instance);
        return 1;
    }
}
