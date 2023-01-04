SOURCE_VERSION = 2

-- Constants
local PUTZLICHT_FRONT_ALIAS = 'putzlicht-front'
local PUTZLICHT_ANBAU_ALIAS = 'putzlicht-anbau'

function setup()
    set_priority(8)
    add_output_alias(PUTZLICHT_FRONT_ALIAS)
    add_output_alias(PUTZLICHT_ANBAU_ALIAS)
end

function tick(now)
    set_alias(PUTZLICHT_ANBAU_ALIAS, HIGH)
    set_alias(PUTZLICHT_FRONT_ALIAS, HIGH)
end