SOURCE_VERSION = 2

-- Constants
local PUTZLICHT_FRONT_ALIAS = 'putzlicht-front'
local PUTZLICHT_ANBAU_ALIAS = 'putzlicht-anbau'

-- Variables
local putzlicht_enabled = false

function setup()
    set_priority(8)
    add_output_alias(PUTZLICHT_FRONT_ALIAS)
    add_output_alias(PUTZLICHT_ANBAU_ALIAS)

    add_event_subscription('button-kitchen-right', EVENT_TYPE_BUTTON_LONG_PRESS, 'handle_long_press')
end

function handle_long_press(address, _typ, duration)
    putzlicht_enabled = not putzlicht_enabled
end

function tick(now)
    set_alias(PUTZLICHT_ANBAU_ALIAS,LOW)
    set_alias(PUTZLICHT_FRONT_ALIAS,LOW)

    if putzlicht_enabled and get_global("global_enable") then
        set_alias(PUTZLICHT_ANBAU_ALIAS, HIGH)
        set_alias(PUTZLICHT_FRONT_ALIAS, HIGH)
    end
end