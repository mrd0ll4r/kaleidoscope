SOURCE_VERSION = 2

-- Constants
local FRONT_DOOR_LIGHT_ALIAS = 'light-outside-front-door'

function setup()
    set_priority(1)

    add_output_alias(FRONT_DOOR_LIGHT_ALIAS)
end

function tick(now)
    set_alias(FRONT_DOOR_LIGHT_ALIAS, LOW)
end