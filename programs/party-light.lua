SOURCE_VERSION = 2

-- Constants
local BLACKLIGHT_ALIAS = 'uv-hauptraum'
local RED_GREEN_LIGHT_ALIAS = 'red-green-party-light'
local MODE_OFF = 0
local MODE_BLACKLIGHT = 1
local MODE_BLACKLIGHT_RED_GREEN = 2
local MODE_RED_GREEN = 3

-- Variables
local blacklight_enabled = false
local red_green_light_enabled = false
local current_mode = MODE_OFF

function setup()
    set_priority(8)
    add_output_alias(BLACKLIGHT_ALIAS)
    add_output_alias(RED_GREEN_LIGHT_ALIAS)

    add_event_subscription('button-kitchen-right', EVENT_TYPE_BUTTON_CLICKED, 'handle_click')
end

function handle_click(address, _typ, duration)
    program_enable('party-light')
    if duration < 1.0 then -- seconds, float
        current_mode = (current_mode + 1) % 4
    end
end

function tick(now)
    set_alias(RED_GREEN_LIGHT_ALIAS,LOW)
    set_alias(BLACKLIGHT_ALIAS,LOW)

    if current_mode == MODE_OFF then
        -- off
    elseif current_mode == MODE_BLACKLIGHT then
        set_alias(BLACKLIGHT_ALIAS, HIGH)
    elseif current_mode == MODE_BLACKLIGHT_RED_GREEN then
        set_alias(BLACKLIGHT_ALIAS, HEAT_2_FAN)
        set_alias(RED_GREEN_LIGHT_ALIAS, HIGH)
    elseif current_mode == MODE_RED_GREEN then
        set_alias(RED_GREEN_LIGHT_ALIAS, HIGH)
    end
end