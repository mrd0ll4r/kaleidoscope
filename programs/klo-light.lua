SOURCE_VERSION = 2

-- Constants
local r = 0
local g = 1
local b = 2
local w = 3
local sine_speed = 0.07
local noise_speed = 0.1
local mode_full_bright = 0
local mode_night = 1

-- Variables
local klo_enabled = true
local light_mode = mode_night

function setup()
    set_priority(3)
    --add_output_group('klo')
    add_output_alias('klo-r')
    add_output_alias('klo-g')
    add_output_alias('klo-b')
    add_output_alias('klo-w')

    add_event_subscription('button-front-door-left', EVENT_TYPE_BUTTON_DOWN, 'handle_down')
    add_event_subscription('button-front-door-left', EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
end

function handle_down(address, _typ)
    program_enable('klo-light')
    klo_enabled = not klo_enabled
    if not klo_enabled then
        light_mode = mode_night
    end
end

function handle_long_press(address, _typ, duration)
    light_mode = mode_full_bright
    klo_enabled = true
end

function compute_white(index, now)
    if light_mode == mode_full_bright then
        return HIGH
    end
    local t = (now - START) * sine_speed
    return map_to_value(0, 1, map_range(-1, 1, 0.7, 0.8, math.sin(t + (math.pi / 4) * index)))
end

function compute_color(index, now)
    if light_mode == mode_full_bright then
        return HIGH
    end
    local t = (now - START) * noise_speed
    return map_to_value(0, 1, map_range(-1, 1, 0.5, 0.9, noise2d(index, t)))
end

function tick(now)
    local global_enabled = get_global("global_enable")
    if global_enabled and klo_enabled then
        set_alias('klo-w', compute_white(w, now))
        set_alias('klo-r', compute_color(r, now))
        set_alias('klo-g', compute_color(g, now))
        set_alias('klo-b', compute_color(b, now))
    else
        set_alias('klo-r', LOW)
        set_alias('klo-g', LOW)
        set_alias('klo-b', LOW)
        set_alias('klo-w', LOW)
    end
end