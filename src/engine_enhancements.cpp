// Selected engine fixes adapted from Lyall/MetaphorFix, Copyright (c) 2024 Lyall.
// Licensed under MIT; see ../licenses/MetaphorFix-MIT.txt.
#define NOMINMAX
#include <Windows.h>
#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstring>
#include <mutex>
#include <sstream>
#include <stdexcept>
#include "engine_enhancements.h"
#include "engine_patterns.h"
#include "../vendor/safetyhook/safetyhook.hpp"

namespace MetaphorEnhancements
{
namespace
{
struct Patch { unsigned char* address; std::vector<unsigned char> before, after; };
struct State
{
    std::mutex mutex;
    bool initialized = false;
    Settings active;
    std::vector<std::string> status;
    std::vector<Patch> patches;
    std::vector<SafetyHookMid> hooks;
    std::vector<std::pair<unsigned char*, size_t>> code;
    std::atomic<bool> intro_skipped = false;
    void (*log)(const char*) = nullptr;
};
// Explicit Shutdown handles dynamic unload. Avoid destructing hooks under process-exit loader lock.
State& state() { static auto* s = new State; return *s; }

void Report(const std::string& message)
{
    state().status.push_back(message);
    if (state().log) state().log(("[ReFantazio Enhanced] " + message).c_str());
}

bool Accessible(const void* pointer, size_t bytes, bool require_writable = false)
{
    MEMORY_BASIC_INFORMATION info{};
    if (!pointer || !VirtualQuery(pointer, &info, sizeof(info)) || info.State != MEM_COMMIT ||
        (info.Protect & (PAGE_GUARD | PAGE_NOACCESS))) return false;
    const auto protection = info.Protect & 0xff;
    if (require_writable && protection != PAGE_READWRITE && protection != PAGE_WRITECOPY &&
        protection != PAGE_EXECUTE_READWRITE && protection != PAGE_EXECUTE_WRITECOPY) return false;
    if (protection != PAGE_READONLY && protection != PAGE_READWRITE && protection != PAGE_WRITECOPY &&
        protection != PAGE_EXECUTE_READ && protection != PAGE_EXECUTE_READWRITE && protection != PAGE_EXECUTE_WRITECOPY)
        return false;
    const auto start = reinterpret_cast<uintptr_t>(pointer);
    const auto end = reinterpret_cast<uintptr_t>(info.BaseAddress) + info.RegionSize;
    return start <= end && bytes <= end - start;
}

bool Executable(const void* pointer)
{
    const auto p = reinterpret_cast<uintptr_t>(pointer);
    for (auto [address, bytes] : state().code)
        if (p >= reinterpret_cast<uintptr_t>(address) && p - reinterpret_cast<uintptr_t>(address) < bytes) return true;
    return false;
}

void DiscoverCode()
{
    auto* base = reinterpret_cast<unsigned char*>(GetModuleHandleW(nullptr));
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    if (!Accessible(dos, sizeof(*dos)) || dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew <= 0)
        throw std::runtime_error("Engine enhancements disabled: invalid game header");
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    if (!Accessible(nt, sizeof(*nt)) || nt->Signature != IMAGE_NT_SIGNATURE ||
        nt->FileHeader.TimeDateStamp != 0x67ebd812 || nt->OptionalHeader.SizeOfImage != 0x14d49000)
        throw std::runtime_error("Engine enhancements disabled: unverified game build");
    auto* section = IMAGE_FIRST_SECTION(nt);
    for (unsigned i = 0; i < nt->FileHeader.NumberOfSections; ++i)
    {
        if (!(section[i].Characteristics & IMAGE_SCN_MEM_EXECUTE)) continue;
        auto* p = base + section[i].VirtualAddress;
        auto* end = p + section[i].Misc.VirtualSize;
        while (p < end)
        {
            MEMORY_BASIC_INFORMATION info{};
            if (!VirtualQuery(p, &info, sizeof(info))) break;
            auto* next = (std::min)(end, static_cast<unsigned char*>(info.BaseAddress) + info.RegionSize);
            if (next <= p) break;
            if (Accessible(p, static_cast<size_t>(next - p))) state().code.emplace_back(p, next - p);
            p = next;
        }
    }
}

unsigned char* Find(const char* pattern)
{
    std::istringstream input(pattern);
    std::string token;
    std::vector<int> bytes;
    while (input >> token) bytes.push_back(token[0] == '?' ? -1 : std::stoi(token, nullptr, 16));
    if (bytes.empty() || bytes[0] < 0) throw std::runtime_error("Invalid engine signature");
    unsigned char* match = nullptr;
    for (auto [start, size] : state().code)
    {
        if (size < bytes.size()) continue;
        auto* limit = start + size - bytes.size() + 1;
        for (auto* p = start; p < limit; ++p)
        {
            p = static_cast<unsigned char*>(std::memchr(p, bytes[0], limit - p));
            if (!p) break;
            bool found = true;
            for (size_t j = 1; j < bytes.size(); ++j)
                if (bytes[j] >= 0 && p[j] != bytes[j]) { found = false; break; }
            if (found)
            {
                if (match) throw std::runtime_error("ambiguous signature; left unchanged");
                match = p;
            }
        }
    }
    if (!match) throw std::runtime_error("signature missing; left unchanged");
    return match;
}

void Write(void* target, const void* data, size_t bytes)
{
    if (!Accessible(target, bytes)) throw std::runtime_error("invalid patch address");
    DWORD previous{};
    if (!VirtualProtect(target, bytes, PAGE_EXECUTE_READWRITE, &previous)) throw std::runtime_error("patch protection failed");
    std::memcpy(target, data, bytes);
    FlushInstructionCache(GetCurrentProcess(), target, bytes);
    DWORD unused{};
    VirtualProtect(target, bytes, previous, &unused);
}

template<class T> void Apply(unsigned char* target, T expected, T value)
{
    if (!Accessible(target, sizeof(T)) || std::memcmp(target, &expected, sizeof(T)))
        throw std::runtime_error("original bytes differ; left unchanged");
    Patch patch{target, std::vector<unsigned char>(sizeof(T)), std::vector<unsigned char>(sizeof(T))};
    std::memcpy(patch.before.data(), target, sizeof(T));
    std::memcpy(patch.after.data(), &value, sizeof(T));
    state().patches.push_back(std::move(patch));
    Write(target, &value, sizeof(T));
}

void Hook(unsigned char* target, safetyhook::MidHookFn callback)
{
    if (!Executable(target)) throw std::runtime_error("hook target is outside executable code");
    auto hook = safetyhook::create_mid(target, callback);
    if (!hook) throw std::runtime_error("hook installation failed");
    state().hooks.push_back(std::move(hook));
}

void Rollback(size_t patches, size_t hooks)
{
    while (state().patches.size() > patches)
    {
        auto& p = state().patches.back();
        if (Accessible(p.address, p.after.size()) && std::memcmp(p.address, p.after.data(), p.after.size()) == 0)
            Write(p.address, p.before.data(), p.before.size());
        state().patches.pop_back();
    }
    while (state().hooks.size() > hooks) state().hooks.pop_back();
}

template<class F> void Feature(const char* name, bool enabled, F apply)
{
    if (!enabled) { Report(std::string(name) + ": original behavior"); return; }
    const auto patches = state().patches.size(), hooks = state().hooks.size();
    try { apply(); Report(std::string(name) + ": active"); }
    catch (const std::exception& e)
    {
        Rollback(patches, hooks);
        Report(std::string(name) + ": disabled (" + e.what() + ")");
    }
}
}

void Sanitize(Settings& s)
{
    if (!std::isfinite(s.gameplay_fov)) s.gameplay_fov = 1.f;
    if (!std::isfinite(s.lod_distance)) s.lod_distance = 20.f;
    s.gameplay_fov = std::clamp(s.gameplay_fov, 0.75f, 1.5f);
    s.lod_distance = std::clamp(s.lod_distance, 10.f, 50.f);
    if (s.shadow_resolution != 2048 && s.shadow_resolution != 4096 && s.shadow_resolution != 8192) s.shadow_resolution = 4096;
}

void Initialize(const Settings& settings, void (*log)(const char*))
{
    auto& s = state();
    std::lock_guard lock(s.mutex);
    if (s.initialized) return;
    s.initialized = true; s.active = settings; Sanitize(s.active); s.log = log;
    if (GetModuleHandleW(L"MetaphorFix.asi") || GetModuleHandleW(L"MetaphorFix.dll"))
    { Report("Engine enhancements disabled: a separate MetaphorFix is loaded"); return; }
    try { DiscoverCode(); }
    catch (const std::exception& e) { Report(e.what()); return; }

    Feature("Intro skipping", s.active.skip_logos || s.active.skip_movie, [] {
        auto* address = Find(Patterns::Intro);
        Hook(address, [](SafetyHookContext& ctx) {
            auto& s = state();
            if (s.intro_skipped.load() || !ctx.rcx) return;
            auto* title = reinterpret_cast<int*>(ctx.rcx + 8);
            if (!Accessible(title, sizeof(int), true)) return;
            if (ctx.rax == 0x30 && s.active.skip_logos)
            { *title = s.active.skip_movie ? 0x43 : 0x3c; s.intro_skipped = true; }
            else if (ctx.rax == 0x3c && s.active.skip_movie)
            { *title = 0x43; s.intro_skipped = true; }
        });
    });
    Feature("Menu FPS fix", s.active.menu_fps, [] {
        Hook(Find(Patterns::MenuFPS), [](SafetyHookContext& ctx) { ctx.rcx = 0; });
    });
    Feature("Shadow resolution", s.active.shadow_resolution != 2048, [] {
        auto* size = Find(Patterns::ShadowSize);
        auto* texel = Find(Patterns::ShadowTexel);
        int32_t relative{}; std::memcpy(&relative, texel + 4, sizeof(relative));
        auto* reciprocal = texel + 8 + relative;
        const float stock_reciprocal = 1.f / 2048.f;
        if (!Accessible(reciprocal, sizeof(float)) || std::memcmp(reciprocal, &stock_reciprocal, sizeof(float)))
            throw std::runtime_error("shadow reciprocal differs");
        Hook(texel + 8, [](SafetyHookContext& ctx) { ctx.xmm3.f32[0] = 1.f / state().active.shadow_resolution; });
        Apply<int>(size + 3, 2048, state().active.shadow_resolution);
        Apply<int>(size + 10, 2048, state().active.shadow_resolution);
        // Preserve cascade coverage; upstream's optional cascade-distance multiplier is omitted.
    });
    Feature("Draw distance", s.active.lod_distance != 10.f, [] {
        auto* lod = Find(Patterns::LOD);
        auto* foliage = Find(Patterns::Foliage);
        int32_t relative{}; std::memcpy(&relative, lod + 4, sizeof(relative));
        auto* constant = lod + 8 + relative;
        Apply<float>(constant, 10000.f, state().active.lod_distance * 1000.f);
        Hook(foliage, [](SafetyHookContext& ctx) { ctx.xmm0.f32[0] = state().active.lod_distance * 1000.f; });
    });
    Feature("Gameplay FOV", s.active.gameplay_fov != 1.f, [] {
        auto* site = Find(Patterns::GameplayFOV);
        if (site[11] != 0xe8) throw std::runtime_error("FOV call opcode differs");
        int32_t relative{}; std::memcpy(&relative, site + 12, sizeof(relative));
        Hook(site + 16 + relative, [](SafetyHookContext& ctx) {
            if (ctx.rax && std::isfinite(ctx.xmm1.f32[0])) ctx.xmm1.f32[0] *= state().active.gameplay_fov;
        });
    });
    Feature("Controller prompts", s.active.force_controller_icons, [] {
        auto* keyboard = Find(Patterns::KeyboardIcons);
        auto* mouse1 = Find(Patterns::MouseIcons1);
        auto* mouse2 = Find(Patterns::MouseIcons2);
        Apply<unsigned char>(keyboard + 10, 2, 0);
        Apply<unsigned char>(mouse1 + 21, 1, 0);
        Apply<unsigned char>(mouse2 + 6, 1, 0);
    });
    Feature("Camera shake removal", s.active.disable_camera_shake, [] {
        Apply<unsigned char>(Find(Patterns::CameraShake) + 3, 5, 4);
    });
}

Settings ActiveSettings() { std::lock_guard lock(state().mutex); return state().active; }
std::vector<std::string> Status() { std::lock_guard lock(state().mutex); return state().status; }
void Shutdown()
{
    auto& s = state(); std::lock_guard lock(s.mutex);
    try { Rollback(0, 0); } catch (...) { /* Unloading: no exception may escape DllMain. */ }
}
}
