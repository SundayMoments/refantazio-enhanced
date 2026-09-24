#pragma once
#include <string>
#include <vector>

namespace MetaphorEnhancements
{
struct Settings
{
    bool skip_logos = true;
    bool skip_movie = false;
    bool menu_fps = true;
    bool disable_camera_shake = false;
    bool force_controller_icons = false;
    float gameplay_fov = 1.f;
    float lod_distance = 20.f;
    int shadow_resolution = 4096;
    bool operator==(const Settings&) const = default;
};

void Sanitize(Settings& settings);
void Initialize(const Settings& settings, void (*log)(const char*));
Settings ActiveSettings();
std::vector<std::string> Status();
void Shutdown();
}
