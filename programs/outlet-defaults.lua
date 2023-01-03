SOURCE_VERSION = 2

function setup()
    set_priority(1)
    set_slow_mode(true)
    add_output_alias('outlet-9-s')
    add_output_alias('outlet-5-s')
    add_output_alias('outlet-3-s')
    add_output_alias('outlet-1-s')
    add_output_alias('outlet-6-s')
end

function tick(now)
    set_alias('outlet-1-s', HIGH)
    set_alias('outlet-3-s', HIGH)
    set_alias('outlet-5-s', HIGH)
    set_alias('outlet-6-s', HIGH)
    set_alias('outlet-9-s', HIGH)
end

