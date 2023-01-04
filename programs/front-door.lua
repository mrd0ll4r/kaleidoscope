SOURCE_VERSION = 2

-- Constants
local MODE_PARAMETER_NAME = "mode"
local MODE_OFF = 0
local MODE_ON = 1
-- Duration to stay on for, in seconds.
local MOTION_DURATION = 30*60
local FRONT_DOOR_LIGHT_ALIAS = 'light-outside-front-door'

-- Variables
local current_mode = MODE_OFF
local on_until = NOW

function setup()
    set_priority(5)

    add_output_alias(FRONT_DOOR_LIGHT_ALIAS)
    declare_discrete_parameter(MODE_PARAMETER_NAME, "whether the light is on or off",
        {build_discrete_parameter_value("On", MODE_ON),
         build_discrete_parameter_value("Off", MODE_OFF)
        }, current_mode, "handle_mode_change")
end

function handle_mode_change(to_mode)
    current_mode = to_mode
    if current_mode == MODE_ON then
        -- turn on for one period
        on_until = NOW + MOTION_DURATION
    end
end

function tick(now)
    set_alias(FRONT_DOOR_LIGHT_ALIAS, LOW)

    if current_mode == MODE_ON and on_until < now then
        set_discrete_parameter_value(MODE_PARAMETER_NAME, MODE_OFF)
        return
    end

    if current_mode == MODE_ON then
        set_alias(FRONT_DOOR_LIGHT_ALIAS, HIGH)
    end
end