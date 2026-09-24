#include "../src/runtime_compatibility.h"
#include <Windows.h>
#include <condition_variable>
#include <cstdio>
#include <mutex>
#include <semaphore>
#include <thread>
#include <vector>

#pragma comment(lib, "Version.lib")

int main()
{
    using namespace RuntimeCompatibility;
    unsigned passed = 0;
    auto check = [&](bool ok, const char* label) {
        std::printf("%s %s\n", ok ? "PASS" : "FAIL", label);
        if (!ok) std::exit(1);
        ++passed;
    };
    check(!Compatible(0), "unknown version rejected");
    check(!Compatible(Version(14, 39, 33523)), "legacy runtime rejected");
    check(!Compatible(Minimum - 1), "older servicing revision rejected");
    check(Compatible(Minimum), "exact required version accepted");
    check(Compatible(Minimum + 1), "newer servicing revision accepted");
    check(Compatible(Version(14, 99, 0)), "newer compatible v14 runtime accepted");
    check(!Compatible(Version(15, 0, 0)), "different ABI major rejected");

    wchar_t path[32768]{};
    const auto module = GetModuleHandleW(L"msvcp140.dll");
    check(module && GetModuleFileNameW(module, path, 32768), "MSVC runtime loaded");
    DWORD ignored = 0;
    const DWORD size = GetFileVersionInfoSizeW(path, &ignored);
    std::vector<unsigned char> resource(size);
    void* value = nullptr;
    UINT length = 0;
    check(size && GetFileVersionInfoW(path, 0, size, resource.data()) &&
        VerQueryValueW(resource.data(), L"\\", &value, &length) &&
        length >= sizeof(VS_FIXEDFILEINFO), "loaded runtime version readable");
    const auto info = static_cast<VS_FIXEDFILEINFO*>(value);
    const uint64_t version = (uint64_t(info->dwFileVersionMS) << 32) | info->dwFileVersionLS;
    check(info->dwSignature == 0xFEEF04BD && Compatible(version), "bundled runtime meets compiled requirement");

    // Exercise the real dynamically linked mutex/condition/semaphore paths that
    // can crash when newer MSVC headers are used with an older app-local DLL.
    std::mutex mutex;
    std::condition_variable condition;
    bool ready = false;
    unsigned value_written = 0;
    std::thread worker([&] {
        { std::lock_guard lock(mutex); value_written = 42; ready = true; }
        condition.notify_one();
    });
    { std::unique_lock lock(mutex); condition.wait(lock, [&] { return ready; }); }
    worker.join();
    check(value_written == 42, "real thread, mutex and condition variable work");
    std::counting_semaphore<1> semaphore(0);
    semaphore.release();
    check(semaphore.try_acquire(), "real semaphore works");
    std::printf("%u runtime checks passed\n%s\n", passed, Requirement);
}
