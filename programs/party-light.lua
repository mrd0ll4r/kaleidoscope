SOURCE_VERSION = 2

-- Constants
local BLACKLIGHT_ALIAS = 'uv-hauptraum'
local RED_GREEN_LIGHT_ALIAS = 'red-green-party-light'

local MODE_PARAMETER_NAME = "mode"
local MODE_OFF = 0
local MODE_BLACKLIGHT = 1
local MODE_BLACKLIGHT_RED_GREEN = 2
local MODE_RED_GREEN = 3

-- Variables
local blacklight_enabled = false
local red_green_light_enabled = false

function setup()
    set_priority(8)
    add_output_alias(BLACKLIGHT_ALIAS)
    add_output_alias(RED_GREEN_LIGHT_ALIAS)

    declare_discrete_parameter(MODE_PARAMETER_NAME, "which parts of the party light to turn on",
        {build_discrete_parameter_value("Off", MODE_OFF),
         build_discrete_parameter_value("Blacklight", MODE_BLACKLIGHT),
         build_discrete_parameter_value("Blacklight+Red/Green", MODE_BLACKLIGHT_RED_GREEN),
         build_discrete_parameter_value("Red/Green", MODE_RED_GREEN)
        }, MODE_OFF, "handle_mode_change")

    -- Turn on the lights, update variables accordingly
    handle_mode_change(MODE_OFF)
end

function handle_mode_change(to)
    if to == MODE_OFF then
        blacklight_enabled = false
        red_green_light_enabled = false
    elseif to == MODE_BLACKLIGHT then
        blacklight_enabled = true
        red_green_light_enabled = false
    elseif to == MODE_BLACKLIGHT_RED_GREEN then
        blacklight_enabled = true
        red_green_light_enabled = true
    elseif to == MODE_RED_GREEN then
        blacklight_enabled = false
        red_green_light_enabled = true
    end
end

function tick(now)
    if blacklight_enabled then
        set_alias(BLACKLIGHT_ALIAS, HIGH)
    end
    if red_green_light_enabled then
        set_alias(RED_GREEN_LIGHT_ALIAS, HIGH)
    end
end