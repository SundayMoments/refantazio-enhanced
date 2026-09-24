// Exercise the real scanner, memory guards, patch rollback and hook library.
#include "../src/engine_enhancements.cpp"
#include <iostream>
#include <limits>

int main()
{
    using namespace MetaphorEnhancements;
    int passed = 0;
    auto check = [&](bool condition, const char* name) {
        if (!condition) throw std::runtime_error(name);
        ++passed; std::cout << "PASS " << name << '\n';
    };
    auto rejected = [&](auto call) { try { call(); return false; } catch (const std::exception&) { return true; } };
    auto* page = static_cast<unsigned char*>(VirtualAlloc(nullptr, 4096, MEM_RESERVE | MEM_COMMIT, PAGE_EXECUTE_READWRITE));
    if (!page) return 1;
    try
    {
        Settings settings;
        settings.gameplay_fov = std::numeric_limits<float>::quiet_NaN();
        settings.lod_distance = std::numeric_limits<float>::infinity();
        settings.shadow_resolution = 123;
        Sanitize(settings);
        check(settings.gameplay_fov == 1.f && settings.lod_distance == 20.f && settings.shadow_resolution == 4096, "invalid settings fall back safely");
        state().code = {{page, 16}};
        page[12] = 0x12; page[13] = 0x34; page[14] = 0x56; page[15] = 0x78;
        check(Find("12 ?? 56 78") == page+12, "signature at final valid byte range");
        check(rejected([&] { Find("12 34 56 78 90"); }), "signature cannot read past code span");
        std::memcpy(page+1, page+12, 4);
        check(rejected([&] { Find("12 ?? 56 78"); }), "ambiguous signature refused");
        check(rejected([&] { Apply<unsigned char>(page, 9, 10); }) && page[0] == 0, "unexpected original bytes refused");
        Apply<unsigned char>(page, 0, 10);
        Rollback(0,0);
        check(page[0] == 0 && state().patches.empty(), "patch rollback restores original");
        Apply<unsigned char>(page, 0, 10); page[0] = 20;
        Rollback(0,0);
        check(page[0] == 20, "rollback preserves another writer's changes");
        DWORD old{};
        VirtualProtect(page,4096,PAGE_READONLY,&old);
        check(Accessible(page,4) && !Accessible(page,4,true), "read-only pointer cannot receive title-state writes");
        VirtualProtect(page,4096,PAGE_NOACCESS,&old);
        check(!Accessible(page,1), "no-access memory refused");
        VirtualProtect(page,4096,PAGE_EXECUTE_READWRITE,&old);
        std::memset(page,0x90,4096);
        const unsigned char function[] = {0x89,0xc8,0x83,0xc0,0x01,0xc3}; // return first_arg + 1
        std::memcpy(page,function,sizeof(function));
        FlushInstructionCache(GetCurrentProcess(),page,4096);
        state().code = {{page,4096}};
        auto call = reinterpret_cast<int(*)(int)>(page);
        check(call(7) == 8, "synthetic function before hook");
        Hook(page, [](SafetyHookContext& ctx) { ctx.rcx = 40; });
        check(call(7) == 41, "real mid-hook changes saved argument register");
        Rollback(0,0);
        check(call(7) == 8 && !std::memcmp(page,function,sizeof(function)), "hook removal restores executable behavior");
        state().code.clear();
        Initialize(Settings{}, nullptr);
        check(Status().size() == 1 && Status()[0].find("unverified game build") != std::string::npos,
            "unsupported executable fails closed without installing hooks");
        VirtualFree(page,0,MEM_RELEASE);
        std::cout << passed << "/13 engine checks passed\n";
    }
    catch (const std::exception& e) { std::cerr << "FAIL " << e.what() << '\n'; return 1; }
}
