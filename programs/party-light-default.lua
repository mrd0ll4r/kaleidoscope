SOURCE_VERSION = 2

-- Constants
local BLACKLIGHT_ALIAS = 'uv-hauptraum'
local RED_GREEN_LIGHT_ALIAS = 'red-green-party-light'

function setup()
    set_priority(1)
    set_slow_mode(true)

    add_output_alias(BLACKLIGHT_ALIAS)
    add_output_alias(RED_GREEN_LIGHT_ALIAS)
end

function tick(now)
    set_alias(RED_GREEN_LIGHT_ALIAS,LOW)
    set_alias(BLACKLIGHT_ALIAS,LOW)
end