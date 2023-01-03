SOURCE_VERSION = 2

function setup()
    set_priority(1)
    set_slow_mode(true)

    add_output_alias('klo-r')
    add_output_alias('klo-g')
    add_output_alias('klo-b')
    add_output_alias('klo-w')
end

function tick(now)
    set_alias('klo-r', LOW)
    set_alias('klo-g', LOW)
    set_alias('klo-b', LOW)
    set_alias('klo-w', LOW)
end