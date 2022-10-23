SOURCE_VERSION = 2

-- Constants
-- Color indices, for noise.
local r = 0
local g = 1
local b = 2
local w = 3

-- Side indices, for noise.
local inner_up = 0
local inner_down = 1
local outer = 2

-- Color/intensity things, mostly used for the night modes.
local sine_speed = 0.07
local noise_speed = 0.3
local white_low = 0.5
local white_high = 0.6
local color_low = 0.3
local color_high = 0.6
local gate_sine_speed = 0.2

-- Program indices
local num_programs = 5
local program_off = 0
local program_night_warm = 1
local program_party = 2
local program_bright = 3
local program_full_bright = 4

-- Variables variables
local program = program_night_warm

function setup()
    set_priority(3)
    add_output_alias('spoider-outer-r')
    add_output_alias('spoider-outer-g')
    add_output_alias('spoider-outer-b')
    add_output_alias('spoider-outer-w')
    add_output_alias('spoider-inner-up-r')
    add_output_alias('spoider-inner-up-g')
    add_output_alias('spoider-inner-up-b')
    add_output_alias('spoider-inner-up-w')
    add_output_alias('spoider-inner-down-r')
    add_output_alias('spoider-inner-down-g')
    add_output_alias('spoider-inner-down-b')
    add_output_alias('spoider-inner-down-w')

    add_event_subscription('button-glassdoor-left', EVENT_TYPE_BUTTON_DOWN, 'change_program')
    add_event_subscription('button-glassdoor-left', EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
end

function change_program(_address, _typ)
    program = (program + 1) % num_programs
end

function handle_long_press(_address, _typ, duration)
    program = program_bright
end

function white_sine(index, now)
    local t = (now - START) * sine_speed
    return map_range(-1, 1, white_low, white_high, math.sin(t + (math.pi / 4) * index))
end

function color_noise(index, side, now)
    return map_range(-1, 1, color_low, color_high, noise3d(index, side, now * noise_speed))
end

function gate_sine(side, now)
    local t = (now - START) * gate_sine_speed
    return clamp(0, 1, map_range(-1, 1, 0, 1, math.sin(t + math.pi * side)) ^ 0.002)
end

function compute_white(side, now)
    local v = map_to_value(0, 1, white_sine(w, now) * gate_sine(side, now))
    --print("white:", v)
    return v
end

function compute_color(index, side, now)
    local v = map_to_value(0, 1, color_noise(index, side, now) * gate_sine(side, now))
    --print("color:", v)
    return v
end

function compute_warm_color(index, side, now)
    local v = color_noise(index, side, now)

    -- Bias colors towards red and green.
    if index == r then
        v = clamp(color_low, color_high, v * 1.2)
    elseif index == g then
        v = clamp(color_low, color_high, v * 1.1)
    else
        v = clamp(color_low, color_high, v * 0.8)
    end

    -- Map to value
    v = v * gate_sine(side, now)
    v = map_to_value(0, 1, v)
    --print("color:", v)
    return v
end

function run_program_bright()
    set_alias('spoider-outer-r', HIGH * 0.8)
    set_alias('spoider-outer-g', HIGH * 0.7)
    set_alias('spoider-outer-b', HIGH * 0.4)
    set_alias('spoider-outer-w', HIGH)
    set_alias('spoider-inner-up-r', HIGH * 0.8)
    set_alias('spoider-inner-up-g', HIGH * 0.8)
    set_alias('spoider-inner-up-b', HIGH * 0.8)
    set_alias('spoider-inner-up-w', HIGH)
    set_alias('spoider-inner-down-r', HIGH * 0.8)
    set_alias('spoider-inner-down-g', HIGH * 0.7)
    set_alias('spoider-inner-down-b', HIGH * 0.4)
    set_alias('spoider-inner-down-w', HIGH)
end

function run_program_full_bright()
    set_alias('spoider-outer-r', HIGH)
    set_alias('spoider-outer-g', HIGH)
    set_alias('spoider-outer-b', HIGH)
    set_alias('spoider-outer-w', HIGH)
    set_alias('spoider-inner-up-r', HIGH)
    set_alias('spoider-inner-up-g', HIGH)
    set_alias('spoider-inner-up-b', HIGH)
    set_alias('spoider-inner-up-w', HIGH)
    set_alias('spoider-inner-down-r', HIGH)
    set_alias('spoider-inner-down-g', HIGH)
    set_alias('spoider-inner-down-b', HIGH)
    set_alias('spoider-inner-down-w', HIGH)
end

function run_program_night_warm(now)
    set_alias('spoider-outer-r', compute_warm_color(r, outer, now))
    set_alias('spoider-outer-g', compute_warm_color(g, outer, now))
    set_alias('spoider-outer-b', compute_warm_color(b, outer, now))
    set_alias('spoider-outer-w', compute_white(outer, now))
    set_alias('spoider-inner-up-r', compute_warm_color(r, inner_up, now))
    set_alias('spoider-inner-up-g', compute_warm_color(g, inner_up, now))
    set_alias('spoider-inner-up-b', compute_warm_color(b, inner_up, now))
    set_alias('spoider-inner-up-w', compute_white(inner_up, now))
    set_alias('spoider-inner-down-r', compute_warm_color(r, inner_down, now))
    set_alias('spoider-inner-down-g', compute_warm_color(g, inner_down, now))
    set_alias('spoider-inner-down-b', compute_warm_color(b, inner_down, now))
    set_alias('spoider-inner-down-w', compute_white(inner_down, now))
end

function run_program_party(now)
    local index = math.floor(now * 10) % 127

    if index < 30 then
        set_alias('spoider-outer-w', compute_white(outer, now))
        set_alias('spoider-inner-up-w', compute_white(inner_up, now))
        set_alias('spoider-inner-down-w', compute_white(inner_down, now))
    end

    if index < 5 then
        set_alias('spoider-outer-r', HIGH)
        set_alias('spoider-inner-up-b', HIGH)
        set_alias('spoider-inner-down-r', HIGH)
    elseif index < 10 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-inner-up-r', HIGH)
        set_alias('spoider-inner-down-g', HIGH)
    elseif index < 15 then
        set_alias('spoider-outer-b', HIGH)
        set_alias('spoider-inner-up-g', HIGH)
        set_alias('spoider-inner-down-b', HIGH)
    elseif index < 20 then
        set_alias('spoider-outer-r', HIGH)
        set_alias('spoider-inner-up-b', HIGH)
        set_alias('spoider-inner-down-r', HIGH)
    elseif index < 25 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-inner-up-r', HIGH)
        set_alias('spoider-inner-down-g', HIGH)
    elseif index < 30 then
        set_alias('spoider-outer-b', HIGH)
        set_alias('spoider-inner-up-g', HIGH)
        set_alias('spoider-inner-down-b', HIGH)

        -- Fast color blinking
    elseif index < 33 then
        set_alias('spoider-outer-r', HIGH)
        set_alias('spoider-inner-up-g', HIGH)
        set_alias('spoider-inner-down-r', HIGH)
    elseif index < 36 then
        -- off
    elseif index < 39 then
        set_alias('spoider-outer-b', HIGH)
        set_alias('spoider-inner-up-g', HIGH)
        set_alias('spoider-inner-down-b', HIGH)
    elseif index < 42 then
        -- off
    elseif index < 45 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-inner-up-r', HIGH)
        set_alias('spoider-inner-down-g', HIGH)
    elseif index < 48 then
        -- off
    elseif index < 51 then
        set_alias('spoider-outer-b', HIGH)
        set_alias('spoider-inner-up-g', HIGH)
        set_alias('spoider-inner-down-r', HIGH)
    elseif index < 54 then
        -- off

        -- Even faster color+white blinking
    elseif index < 57 then
        set_alias('spoider-outer-r', HIGH)
        set_alias('spoider-outer-w', HIGH)
        set_alias('spoider-inner-up-g', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
        set_alias('spoider-inner-down-b', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 59 then
        set_alias('spoider-outer-w', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 61 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-outer-w', HIGH)
        set_alias('spoider-inner-up-b', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
        set_alias('spoider-inner-down-r', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 63 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-inner-up-b', HIGH)
        set_alias('spoider-inner-down-r', HIGH)
    elseif index < 65 then
        set_alias('spoider-outer-b', HIGH)
        set_alias('spoider-outer-w', HIGH)
        set_alias('spoider-inner-up-r', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
        set_alias('spoider-inner-down-g', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 67 then
        set_alias('spoider-outer-w', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 69 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-outer-w', HIGH)
        set_alias('spoider-inner-up-b', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
        set_alias('spoider-inner-down-r', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 71 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-inner-up-b', HIGH)
        set_alias('spoider-inner-down-r', HIGH)
    elseif index < 73 then
        set_alias('spoider-outer-b', HIGH)
        set_alias('spoider-outer-w', HIGH)
        set_alias('spoider-inner-up-r', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
        set_alias('spoider-inner-down-g', HIGH)
        set_alias('spoider-inner-down-w', HIGH)

        -- Super fast single color+white blinking, per stripe
        -- outer stripe
    elseif index < 75 then
        set_alias('spoider-outer-r', HIGH)
    elseif index < 76 then
        set_alias('spoider-outer-r', HIGH)
        set_alias('spoider-outer-w', HIGH)
    elseif index < 77 then
        set_alias('spoider-outer-g', HIGH)
    elseif index < 78 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-outer-w', HIGH)
    elseif index < 79 then
        set_alias('spoider-outer-b', HIGH)
    elseif index < 80 then
        set_alias('spoider-outer-b', HIGH)
        set_alias('spoider-outer-w', HIGH)
        -- inner down stripe
    elseif index < 81 then
        set_alias('spoider-inner-down-r', HIGH)
    elseif index < 82 then
        set_alias('spoider-inner-down-r', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 83 then
        set_alias('spoider-inner-down-g', HIGH)
    elseif index < 84 then
        set_alias('spoider-inner-down-g', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 85 then
        set_alias('spoider-inner-down-b', HIGH)
    elseif index < 86 then
        set_alias('spoider-inner-down-b', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
        -- inner up stripe
    elseif index < 87 then
        set_alias('spoider-inner-up-r', HIGH)
    elseif index < 88 then
        set_alias('spoider-inner-up-r', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
    elseif index < 89 then
        set_alias('spoider-inner-up-g', HIGH)
    elseif index < 90 then
        set_alias('spoider-inner-up-g', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
    elseif index < 91 then
        set_alias('spoider-inner-up-b', HIGH)
    elseif index < 92 then
        set_alias('spoider-inner-up-b', HIGH)
        set_alias('spoider-inner-up-w', HIGH)


        -- The same, but half speed
        -- outer stripe
    elseif index < 93 then
        set_alias('spoider-outer-r', HIGH)
    elseif index < 95 then
        set_alias('spoider-outer-r', HIGH)
        set_alias('spoider-outer-w', HIGH)
    elseif index < 97 then
        set_alias('spoider-outer-g', HIGH)
    elseif index < 99 then
        set_alias('spoider-outer-g', HIGH)
        set_alias('spoider-outer-w', HIGH)
    elseif index < 101 then
        set_alias('spoider-outer-b', HIGH)
    elseif index < 103 then
        set_alias('spoider-outer-b', HIGH)
        set_alias('spoider-outer-w', HIGH)
        -- inner down stripe
    elseif index < 105 then
        set_alias('spoider-inner-down-r', HIGH)
    elseif index < 107 then
        set_alias('spoider-inner-down-r', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 109 then
        set_alias('spoider-inner-down-g', HIGH)
    elseif index < 111 then
        set_alias('spoider-inner-down-g', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
    elseif index < 113 then
        set_alias('spoider-inner-down-b', HIGH)
    elseif index < 115 then
        set_alias('spoider-inner-down-b', HIGH)
        set_alias('spoider-inner-down-w', HIGH)
        -- inner up stripe
    elseif index < 117 then
        set_alias('spoider-inner-up-r', HIGH)
    elseif index < 119 then
        set_alias('spoider-inner-up-r', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
    elseif index < 121 then
        set_alias('spoider-inner-up-g', HIGH)
    elseif index < 123 then
        set_alias('spoider-inner-up-g', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
    elseif index < 125 then
        set_alias('spoider-inner-up-b', HIGH)
    elseif index < 127 then
        set_alias('spoider-inner-up-b', HIGH)
        set_alias('spoider-inner-up-w', HIGH)
    end
end

function tick(now)
    local global_enabled = get_global("global_enable")

    set_alias('spoider-outer-r', LOW)
    set_alias('spoider-outer-g', LOW)
    set_alias('spoider-outer-b', LOW)
    set_alias('spoider-outer-w', LOW)
    set_alias('spoider-inner-up-r', LOW)
    set_alias('spoider-inner-up-g', LOW)
    set_alias('spoider-inner-up-b', LOW)
    set_alias('spoider-inner-up-w', LOW)
    set_alias('spoider-inner-down-r', LOW)
    set_alias('spoider-inner-down-g', LOW)
    set_alias('spoider-inner-down-b', LOW)
    set_alias('spoider-inner-down-w', LOW)

    if global_enabled then
        if program == program_off then
            -- off
        elseif program == program_full_bright then
            run_program_full_bright()
        elseif program == program_bright then
            run_program_bright()
        elseif program == program_night_warm then
            run_program_night_warm(now)
        elseif program == program_party then
            run_program_party(now)
        end
    end
end