#pragma once

#include <cstdint>
#include <crtversion.h>

#define RE_STRINGIFY_INNER(value) #value
#define RE_STRINGIFY(value) RE_STRINGIFY_INNER(value)

namespace RuntimeCompatibility
{
    constexpr uint64_t Version(unsigned major, unsigned minor, unsigned build, unsigned revision = 0)
    {
        return (uint64_t(major) << 48) | (uint64_t(minor) << 32) | (uint64_t(build) << 16) | revision;
    }

    // Compile this requirement into the addon so the packager can validate the
    // actual binary's dependency floor even when packaging on a different PC.
    inline constexpr char Requirement[] = "RE_MSVC_RUNTIME_MIN="
        RE_STRINGIFY(_VC_CRT_MAJOR_VERSION) "." RE_STRINGIFY(_VC_CRT_MINOR_VERSION) "."
        RE_STRINGIFY(_VC_CRT_BUILD_VERSION) "." RE_STRINGIFY(_VC_CRT_RBUILD_VERSION);
    inline constexpr uint64_t Minimum = Version(_VC_CRT_MAJOR_VERSION, _VC_CRT_MINOR_VERSION,
        _VC_CRT_BUILD_VERSION, _VC_CRT_RBUILD_VERSION);

    constexpr bool Compatible(uint64_t version)
    {
        return (version >> 48) == _VC_CRT_MAJOR_VERSION && version >= Minimum;
    }
}

#undef RE_STRINGIFY
#undef RE_STRINGIFY_INNER
