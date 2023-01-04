SOURCE_VERSION = 2

-- Constants
local GLOBAL_ENABLE_LONG_PRESS_SECONDS = 3
local GLOBAL_ENABLE_PROGRAMS_TO_TURN_OFF = {'kitchen-light', 'bug-catcher', 'klo-light', 'lichterketten', 'spoider', 'party-light', 'putzlicht'}
local GLOBAL_ENABLE_PROGRAMS_TO_TURN_ON = {'kitchen-light', 'bug-catcher', 'klo-light', 'lichterketten', 'spoider'}

local BUTTON_FRONT_DOOR_RIGHT_ALIAS = 'button-front-door-right'
local BUTTON_FRONT_DOOR_RIGHT_ADDRESS = input_alias_to_address(BUTTON_FRONT_DOOR_RIGHT_ALIAS)

local BUTTON_FRONT_DOOR_LEFT_ALIAS = 'button-front-door-left'
local BUTTON_FRONT_DOOR_LEFT_ADDRESS = input_alias_to_address(BUTTON_FRONT_DOOR_LEFT_ALIAS)

local BUTTON_GLASS_DOOR_RIGHT_ALIAS = 'button-glassdoor-right'
local BUTTON_GLASS_DOOR_RIGHT_ADDRESS = input_alias_to_address(BUTTON_GLASS_DOOR_RIGHT_ALIAS)

local BUTTON_GLASS_DOOR_LEFT_ALIAS = 'button-glassdoor-left'
local BUTTON_GLASS_DOOR_LEFT_ADDRESS = input_alias_to_address(BUTTON_GLASS_DOOR_LEFT_ALIAS)

local BUTTON_KITCHEN_RIGHT_ALIAS = 'button-kitchen-right'
local BUTTON_KITCHEN_RIGHT_ADDRESS = input_alias_to_address(BUTTON_KITCHEN_RIGHT_ALIAS)

local BUTTON_KITCHEN_LEFT_ALIAS = 'button-kitchen-left'
local BUTTON_KITCHEN_LEFT_ADDRESS = input_alias_to_address(BUTTON_KITCHEN_LEFT_ALIAS)

local BUTTON_BEDROOM_LEFT_ALIAS = 'button-bedroom-left'
local BUTTON_BEDROOM_LEFT_ADDRESS = input_alias_to_address(BUTTON_BEDROOM_LEFT_ALIAS)

local BUTTON_BEDROOM_RIGHT_ALIAS = 'button-bedroom-right'
local BUTTON_BEDROOM_RIGHT_ADDRESS = input_alias_to_address(BUTTON_BEDROOM_RIGHT_ALIAS)

-- Variables
local global_enable_state = true

function setup()
    -- Priority does not matter, we don't need to run tick(), only handle events.
    set_priority(1)

    add_event_subscription(BUTTON_FRONT_DOOR_RIGHT_ALIAS, EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
    add_event_subscription(BUTTON_FRONT_DOOR_RIGHT_ALIAS, EVENT_TYPE_BUTTON_CLICKED, 'handle_click')

    add_event_subscription(BUTTON_FRONT_DOOR_LEFT_ALIAS, EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
    add_event_subscription(BUTTON_FRONT_DOOR_LEFT_ALIAS, EVENT_TYPE_BUTTON_CLICKED, 'handle_click')

    add_event_subscription(BUTTON_KITCHEN_LEFT_ALIAS, EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
    add_event_subscription(BUTTON_KITCHEN_LEFT_ALIAS, EVENT_TYPE_BUTTON_CLICKED, 'handle_click')

    add_event_subscription(BUTTON_KITCHEN_RIGHT_ALIAS, EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
    add_event_subscription(BUTTON_KITCHEN_RIGHT_ALIAS, EVENT_TYPE_BUTTON_CLICKED, 'handle_click')

    add_event_subscription(BUTTON_GLASS_DOOR_LEFT_ALIAS, EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
    add_event_subscription(BUTTON_GLASS_DOOR_LEFT_ALIAS, EVENT_TYPE_BUTTON_CLICKED, 'handle_click')

    add_event_subscription(BUTTON_GLASS_DOOR_RIGHT_ALIAS, EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
    add_event_subscription(BUTTON_GLASS_DOOR_RIGHT_ALIAS, EVENT_TYPE_BUTTON_CLICKED, 'handle_click')
    
    --add_event_subscription(BUTTON_BEDROOM_LEFT_ALIAS, EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
    add_event_subscription(BUTTON_BEDROOM_LEFT_ALIAS, EVENT_TYPE_BUTTON_CLICKED, 'handle_click')

    --add_event_subscription(BUTTON_BEDROOM_RIGHT_ALIAS, EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
    add_event_subscription(BUTTON_BEDROOM_RIGHT_ALIAS, EVENT_TYPE_BUTTON_CLICKED, 'handle_click')
end

function is_long_click(duration)
    return duration >= 1.0
end

function handle_click(address, _typ, duration)
    if is_long_click(duration) then
        return
    end

    if address == BUTTON_FRONT_DOOR_LEFT_ADDRESS then
        program_enable_toggle('klo-light')
        set_foreign_discrete_parameter_value('klo-light', 'brightness', 0)
    elseif address == BUTTON_FRONT_DOOR_RIGHT_ADDRESS then
        program_enable('front-door')
        increment_foreign_discrete_parameter_value('front-door', 'mode', 1)
    elseif address == BUTTON_GLASS_DOOR_LEFT_ADDRESS then
        program_enable_toggle('lichterketten')
        set_foreign_discrete_parameter_value('lichterketten', 'brightness', 0)
    elseif address == BUTTON_GLASS_DOOR_RIGHT_ADDRESS then
        program_enable('spoider')
        increment_foreign_discrete_parameter_value('spoider', 'mode', 1)
    elseif address == BUTTON_KITCHEN_LEFT_ADDRESS then
        program_enable('kitchen-light')
        increment_foreign_discrete_parameter_value('kitchen-light', 'rings', 1)
    elseif address == BUTTON_KITCHEN_RIGHT_ADDRESS then
        program_enable('party-light')
        increment_foreign_discrete_parameter_value('party-light', 'mode', 1)
    elseif address == BUTTON_BEDROOM_RIGHT_ADDRESS then
        program_enable('heating')
        increment_foreign_discrete_parameter_value('heating', 'air-in-heating', 1)
    elseif address == BUTTON_BEDROOM_LEFT_ADDRESS then
        program_enable('heating')
        increment_foreign_discrete_parameter_value('heating', 'air-out', 1)
    end
end

function handle_long_press(address, _typ, duration)
    if address == BUTTON_FRONT_DOOR_RIGHT_ADDRESS then
        if duration >= GLOBAL_ENABLE_LONG_PRESS_SECONDS then
            global_enable_state = not global_enable_state
            if global_enable_state then
                for _, p in ipairs(GLOBAL_ENABLE_PROGRAMS_TO_TURN_ON) do
                    program_enable(p)
                end
            else
                for _, p in ipairs(GLOBAL_ENABLE_PROGRAMS_TO_TURN_OFF) do
                    program_disable(p)
                end
            end
        end
    elseif address == BUTTON_FRONT_DOOR_LEFT_ADDRESS then
        program_enable('klo-light')
        set_foreign_discrete_parameter_value('klo-light', 'brightness', 1)
    elseif address == BUTTON_GLASS_DOOR_LEFT_ADDRESS then
        program_enable('lichterketten')
        set_foreign_discrete_parameter_value('lichterketten', 'brightness', 1)
    elseif address == BUTTON_GLASS_DOOR_RIGHT_ADDRESS then
        program_enable('spoider')
        set_foreign_discrete_parameter_value('spoider', 'mode', 2)
    elseif address == BUTTON_KITCHEN_LEFT_ADDRESS then
        program_enable('kitchen-light')
        increment_foreign_discrete_parameter_value('kitchen-light', 'brightness', 1)
    elseif address == BUTTON_KITCHEN_RIGHT_ADDRESS then
        program_enable_toggle('putzlicht')
    end
end

function tick(now) end