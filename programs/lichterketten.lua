SOURCE_VERSION = 2

-- Constants
local mode_full_bright = 0
local mode_night = 1
local noise_speed = 0.1
local KETTE_1_OUTLET = 'outlet-2-s1'
local KETTE_2_OUTLET = 'outlet-2-s2'
local KETTE_3_OUTLET = 'outlet-4-s1'
local KETTE_4_OUTLET = 'outlet-8-s2'

local MODE_BRIGHTNESS_NAME = "brightness"
local MODE_BRIGHTNESS_NIGHT = 0
local MODE_BRIGHTNESS_FULL_BRIGHT = 1

-- Variables
local current_brightness_mode = MODE_BRIGHTNESS_FULL_BRIGHT

function setup()
    set_priority(3)
    add_output_alias(KETTE_1_OUTLET)
    add_output_alias(KETTE_2_OUTLET)
    add_output_alias(KETTE_3_OUTLET)
    add_output_alias(KETTE_4_OUTLET)

    -- Declare parameters
    declare_discrete_parameter(MODE_BRIGHTNESS_NAME, "day/night brightness mode",
        {build_discrete_parameter_value("Night", MODE_BRIGHTNESS_NIGHT),
         build_discrete_parameter_value("Full Bright", MODE_BRIGHTNESS_FULL_BRIGHT)
        }, current_brightness_mode, "handle_brightness_mode_change")
end

function handle_brightness_mode_change(to)
    current_brightness_mode=to
end

function tick(now)
    local kette1_on = true
    local kette2_on = (math.floor(now/30) % 2 == 1)
    local kette3_on = (math.floor(now/30) % 2 == 0)
    local kette4_on = noise2d(4, now*noise_speed) > 0

    if current_brightness_mode == MODE_BRIGHTNESS_NIGHT then
        if kette1_on then
            set_alias(KETTE_1_OUTLET, HIGH)
        end
        if kette2_on then
            set_alias(KETTE_2_OUTLET, HIGH)
        end
        if kette3_on then
            set_alias(KETTE_3_OUTLET, HIGH)
        end
        if kette4_on then
            set_alias(KETTE_4_OUTLET, HIGH)
        end
     else
        set_alias(KETTE_1_OUTLET, HIGH)
        set_alias(KETTE_2_OUTLET, HIGH)
        set_alias(KETTE_3_OUTLET, HIGH)
        set_alias(KETTE_4_OUTLET, HIGH)
    end
end
