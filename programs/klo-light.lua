SOURCE_VERSION = 2

-- Constants
local r = 0
local g = 1
local b = 2
local w = 3
local sine_speed = 0.07
local noise_speed = 0.1

local MODE_BRIGHTNESS_NAME = "brightness"
local MODE_BRIGHTNESS_NIGHT = 0
local MODE_BRIGHTNESS_FULL_BRIGHT = 1

-- Variables
local current_brightness_mode = MODE_BRIGHTNESS_NIGHT

function setup()
    set_priority(3)
    --add_output_group('klo')
    add_output_alias('klo-r')
    add_output_alias('klo-g')
    add_output_alias('klo-b')
    add_output_alias('klo-w')

    -- Declare parameters
    declare_discrete_parameter(MODE_BRIGHTNESS_NAME, "day/night brightness mode",
        {build_discrete_parameter_value("Night", MODE_BRIGHTNESS_NIGHT),
         build_discrete_parameter_value("Full Bright", MODE_BRIGHTNESS_FULL_BRIGHT)
        }, current_brightness_mode, "handle_brightness_mode_change")

end

function handle_brightness_mode_change(to)
    current_brightness_mode=to
end

function compute_white(index, now)
    if current_brightness_mode == MODE_BRIGHTNESS_FULL_BRIGHT then
        return HIGH
    end
    local t = (now - START) * sine_speed
    return map_to_value(0, 1, map_range(-1, 1, 0.7, 0.8, math.sin(t + (math.pi / 4) * index)))
end

function compute_color(index, now)
    if current_brightness_mode == MODE_BRIGHTNESS_FULL_BRIGHT then
        return HIGH
    end
    local t = (now - START) * noise_speed
    return map_to_value(0, 1, map_range(-1, 1, 0.5, 0.9, noise2d(index, t)))
end

function tick(now)
    set_alias('klo-w', compute_white(w, now))
    set_alias('klo-r', compute_color(r, now))
    set_alias('klo-g', compute_color(g, now))
    set_alias('klo-b', compute_color(b, now))
end