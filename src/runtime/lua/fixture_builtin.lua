-- ================ SETUP ================
-- These are provided by the runtime during setup

-- Set a name for the fixture.
-- This must be unique in the Kaleidoscope instance.
function fixture_name(name) end

-- Add an alias to the list of outputs of this Fixture.
-- Fixture outputs must not overlap.
function add_output_alias(name) end

-- Add a program to the list of available programs for this fixture.
-- The name must be unique among programs for this fixture.
-- The program source is loaded from the provided path.
function add_program(program_name, program_source_path) end

-- Control whether the builtin programs ON and OFF should be disabled.
function disable_builtin_programs(b) end

-- Control whether the builtin program MANUAL for manual output control should be disabled.
function disable_manual_program(b) end