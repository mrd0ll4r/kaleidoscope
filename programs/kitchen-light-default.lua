SOURCE_VERSION = 2

function setup()
    set_priority(1)
    set_slow_mode(true)

    add_output_alias('kitchen-innen-r')
    add_output_alias('kitchen-innen-g')
    add_output_alias('kitchen-innen-b')
    add_output_alias('kitchen-innen-w')

    add_output_alias('kitchen-out-r')
    add_output_alias('kitchen-out-g')
    add_output_alias('kitchen-out-b')
    add_output_alias('kitchen-out-w')

    add_output_alias('kitchen-spots')
end

function tick(now)
    set_alias('kitchen-innen-r', LOW)
    set_alias('kitchen-innen-g', LOW)
    set_alias('kitchen-innen-b', LOW)
    set_alias('kitchen-innen-w', LOW)

    set_alias('kitchen-out-r', LOW)
    set_alias('kitchen-out-g', LOW)
    set_alias('kitchen-out-b', LOW)
    set_alias('kitchen-out-w', LOW)

    set_alias('kitchen-spots', LOW)
end