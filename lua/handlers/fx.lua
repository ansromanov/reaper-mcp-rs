-- FX, parameter, pin and FX-chain handlers.
-- Loaded by reaper_mcp_handlers.lua with the shared helpers and the handlers table.

local S, handlers = ...

local file_exists, find_param, fx_name, require_int, resolve_fx, resolve_track, undo = S.file_exists, S.find_param, S.fx_name, S.require_int, S.resolve_fx, S.resolve_track, S.undo

local function describe_param(track, fx, i)
  local _, name = reaper.TrackFX_GetParamName(track, fx, i, "")
  local _, formatted = reaper.TrackFX_GetFormattedParamValue(track, fx, i, "")
  local _, minimum, maximum = reaper.TrackFX_GetParam(track, fx, i)
  return {
    index = i,
    name = name,
    normalized = reaper.TrackFX_GetParamNormalized(track, fx, i),
    formatted = formatted,
    min = minimum,
    max = maximum,
  }
end

handlers.list_fx = function(args)
  local track = resolve_track(args.track)
  local fx = {}
  for i = 0, reaper.TrackFX_GetCount(track) - 1 do
    fx[#fx + 1] = {
      index = i,
      name = fx_name(track, i),
      enabled = reaper.TrackFX_GetEnabled(track, i),
      param_count = reaper.TrackFX_GetNumParams(track, i),
    }
  end
  return fx
end

handlers.add_fx = function(args)
  local track = resolve_track(args.track)
  if type(args.name) ~= "string" or args.name == "" then error("name must be a non-empty string") end
  return undo("add FX", function()
    local index = reaper.TrackFX_AddByName(track, args.name, false, -1)
    if index < 0 then error("FX not found or could not be loaded: " .. args.name) end
    return { index = index, name = fx_name(track, index), param_count = reaper.TrackFX_GetNumParams(track, index) }
  end)
end

handlers.remove_fx = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  return undo("remove FX", function()
    local name = fx_name(track, fx)
    if not reaper.TrackFX_Delete(track, fx) then error("REAPER refused to delete the FX") end
    return { removed = name }
  end)
end

handlers.set_fx_enabled = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  if type(args.enabled) ~= "boolean" then error("enabled must be a boolean") end
  return undo("enable FX", function()
    reaper.TrackFX_SetEnabled(track, fx, args.enabled)
    return { index = fx, name = fx_name(track, fx), enabled = reaper.TrackFX_GetEnabled(track, fx) }
  end)
end

handlers.list_fx_params = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local params = {}
  for i = 0, reaper.TrackFX_GetNumParams(track, fx) - 1 do
    params[#params + 1] = describe_param(track, fx, i)
  end
  return { fx = fx_name(track, fx), params = params }
end

-- Number shown by a plugin for a parameter, for example "-18.0 dB" -> -18,
-- "1.2 kHz" -> 1200, "4.0:1" -> 4.
local INFINITY_SIGN = utf8.char(8734)

local NO_BREAK_SPACE = utf8.char(160)

local NARROW_NO_BREAK_SPACE = utf8.char(8239)

local MINUS_SIGN = utf8.char(8722)

local function parse_display_number(text)
  -- Plugins print infinity as a symbol; that is not a number to solve for.
  if text:find(INFINITY_SIGN, 1, true) or text:lower():find("inf", 1, true) then return nil end
  -- Normalize non-breaking spaces and the Unicode minus (kHs plugins print both); take the unit that follows the number
  -- (plugins sometimes print a second, stale unit: "2.50 s ms").
  local lowered = text:lower():gsub(NO_BREAK_SPACE, " "):gsub(NARROW_NO_BREAK_SPACE, " "):gsub(MINUS_SIGN, "-")
  local number, unit = lowered:match("(-?%d+%.?%d*)%s*(%a*)")
  if not number then return nil end
  local value = tonumber(number)
  if unit == "khz" then
    value = value * 1000
  elseif unit == "s" or unit == "sec" then
    -- Seconds shown for long times: normalize to ms.
    value = value * 1000
  end
  return value
end

local function format_normalized(track, fx, param, normalized)
  local ok, text = reaper.TrackFX_FormatParamValueNormalized(track, fx, param, normalized, "")
  if not ok then return nil end
  return text
end

-- Finds the normalized value whose displayed number matches `target` by
-- bisection (assumes the display is monotonic in the normalized value).
local function solve_display_number(track, fx, param, target)
  local function shown(normalized)
    local text = format_normalized(track, fx, param, normalized)
    return text and parse_display_number(text)
  end
  -- Some parameters show text at an end of the range (for example an infinite
  -- ratio); use the outermost numeric points instead.
  local lo_bound, hi_bound = 0.0, 1.0
  local low, high = shown(0), shown(1)
  if high == nil then
    for _, probe in ipairs({ 0.995, 0.99, 0.98, 0.95, 0.9 }) do
      high = shown(probe)
      if high ~= nil then hi_bound = probe break end
    end
  end
  if low == nil then
    for _, probe in ipairs({ 0.005, 0.01, 0.02, 0.05, 0.1 }) do
      low = shown(probe)
      if low ~= nil then lo_bound = probe break end
    end
  end
  if low == nil or high == nil or low == high then
    error(string.format("parameter does not show a numeric value at its ends (low %s, high %s, texts '%s' / '%s'); use display_text or value",
      tostring(low), tostring(high), tostring(format_normalized(track, fx, param, 0)),
      tostring(format_normalized(track, fx, param, 1))))
  end
  local ascending = high > low
  local lo, hi = lo_bound, hi_bound
  for _ = 1, 40 do
    local mid = (lo + hi) / 2
    local current = shown(mid)
    if current == nil then error("parameter display could not be parsed") end
    if (current < target) == ascending then lo = mid else hi = mid end
  end
  -- Of the two values around the crossing, take the one that displays closer.
  local best, achieved = lo, shown(lo)
  local upper = shown(hi)
  if achieved == nil or (upper ~= nil and math.abs(upper - target) < math.abs(achieved - target)) then
    best, achieved = hi, upper
  end
  local tolerance = math.max(math.abs(high - low) * 0.01, 0.06)
  if achieved == nil or math.abs(achieved - target) > tolerance then
    error(string.format("display value %s is outside the parameter range (%s to %s; end texts '%s' / '%s')",
      tostring(target), tostring(low), tostring(high),
      tostring(format_normalized(track, fx, param, lo_bound)), tostring(format_normalized(track, fx, param, hi_bound))))
  end
  return best
end

-- Finds a normalized value whose displayed text equals `wanted`
-- (case-insensitive), for choice and switch parameters.
local function solve_display_text(track, fx, param, wanted)
  local steps = 1000
  local matches = {}
  local wanted_lower = wanted:lower()
  for i = 0, steps do
    local normalized = i / steps
    local text = format_normalized(track, fx, param, normalized)
    if text and text:lower() == wanted_lower then matches[#matches + 1] = normalized end
  end
  if #matches == 0 then error("no value of the parameter displays '" .. wanted .. "'") end
  return matches[math.ceil(#matches / 2)]
end

local function solve_target(track, fx, param, spec)
  local given = (spec.value ~= nil and 1 or 0) + (spec.display_value ~= nil and 1 or 0)
    + (spec.display_text ~= nil and 1 or 0)
  if given ~= 1 then error("pass exactly one of value, display_value or display_text") end
  if spec.value ~= nil then
    if type(spec.value) ~= "number" then error("value must be a number") end
    return spec.value
  elseif spec.display_value ~= nil then
    if type(spec.display_value) ~= "number" then error("display_value must be a number") end
    return solve_display_number(track, fx, param, spec.display_value)
  end
  if type(spec.display_text) ~= "string" then error("display_text must be a string") end
  return solve_display_text(track, fx, param, spec.display_text)
end

handlers.set_fx_param = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local param = find_param(track, fx, args.param)
  return undo("set FX parameter", function()
    local normalized = solve_target(track, fx, param, args)
    if not reaper.TrackFX_SetParamNormalized(track, fx, param, normalized) then
      error("FX rejected the parameter value")
    end
    return describe_param(track, fx, param)
  end)
end

-- Applies many parameters of one FX in a single undo block. Failures are
-- reported per entry and do not stop the rest.
handlers.set_fx_params = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  if type(args.params) ~= "table" then error("params must be a list") end
  return undo("set FX parameters", function()
    local results = {}
    for _, entry in ipairs(args.params) do
      local ok, result = pcall(function()
        local param = find_param(track, fx, entry.param)
        local normalized = solve_target(track, fx, param, entry)
        if not reaper.TrackFX_SetParamNormalized(track, fx, param, normalized) then
          error("FX rejected the parameter value")
        end
        return describe_param(track, fx, param)
      end)
      if ok then
        results[#results + 1] = { ok = true, name = result.name, formatted = result.formatted }
      else
        results[#results + 1] = { ok = false, param = entry.param, error = tostring(result) }
      end
    end
    return { fx = fx_name(track, fx), results = results }
  end)
end

handlers.set_fx_preset = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  if type(args.preset) ~= "string" or args.preset == "" then error("preset must be a non-empty string") end
  return undo("set FX preset", function()
    if not reaper.TrackFX_SetPreset(track, fx, args.preset) then
      error("preset not found: " .. args.preset)
    end
    local _, name = reaper.TrackFX_GetPreset(track, fx, "")
    return { fx = fx_name(track, fx), preset = name }
  end)
end

handlers.list_fx_presets = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local _, current = reaper.TrackFX_GetPreset(track, fx, "")
  local _, count = reaper.TrackFX_GetPresetIndex(track, fx)
  return { fx = fx_name(track, fx), current = current, count = count }
end

-- Moves an FX to another position in the same chain (to_index is its final index).
handlers.move_fx = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local target = require_int(args.to_index, "to_index", 0)
  local count = reaper.TrackFX_GetCount(track)
  if target >= count then error("to_index is out of range") end
  return undo("move FX", function()
    if target ~= fx then
      -- CopyToTrack with is_move=true inserts before the destination index.
      local destination = target > fx and target + 1 or target
      reaper.TrackFX_CopyToTrack(track, fx, track, destination, true)
    end
    local fx_list = {}
    for i = 0, reaper.TrackFX_GetCount(track) - 1 do fx_list[#fx_list + 1] = fx_name(track, i) end
    return { fx = fx_list }
  end)
end

-- Distinct displayed texts of a parameter across its range, for choice
-- parameters (shapes, modes, styles). Returns each text with the first
-- normalized value that shows it.
handlers.list_fx_param_choices = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local param = find_param(track, fx, args.param)
  local steps = args.steps or 400
  local choices, seen = {}, {}
  for i = 0, steps do
    local normalized = i / steps
    local text = format_normalized(track, fx, param, normalized)
    if text and not seen[text] then
      seen[text] = true
      choices[#choices + 1] = { text = text, normalized = normalized }
      if #choices > 64 then break end
    end
  end
  local _, name = reaper.TrackFX_GetParamName(track, fx, param, "")
  return { param = name, index = param, choices = choices }
end

-- Text a plugin shows for a normalized parameter value, without changing it.
handlers.format_fx_param = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local param = find_param(track, fx, args.param)
  if type(args.value) ~= "number" then error("value must be a number") end
  return { param = param, value = args.value, formatted = format_normalized(track, fx, param, args.value) }
end

-- Replaces the automation envelope of an FX parameter. Point values are
-- normalized 0..1 plugin values; shape follows REAPER (0 linear, 1 square,
-- 2 slow start/end, 3 fast start, 4 fast end, 5 bezier).
handlers.set_fx_envelope = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local param = find_param(track, fx, args.param)
  if type(args.points) ~= "table" then error("points must be a list") end
  return undo("set FX envelope", function()
    local envelope = reaper.GetFXEnvelope(track, fx, param, true)
    if not envelope then error("could not create the FX envelope") end
    reaper.DeleteEnvelopePointRange(envelope, -1e9, 1e9)
    for _, point in ipairs(args.points) do
      if type(point.time) ~= "number" or type(point.value) ~= "number" then
        error("each point needs numeric time and value")
      end
      reaper.InsertEnvelopePoint(envelope, point.time, point.value, point.shape or 0, point.tension or 0, false, true)
    end
    reaper.Envelope_SortPoints(envelope)
    return { param = param, points = reaper.CountEnvelopePoints(envelope) }
  end)
end

-- Pin mappings route track channels to FX pins. Each pin has a bitmask of
-- track channels (bit 0 = channel 1, bit 1 = channel 2, ...), low 32 bits only.
handlers.get_fx_pins = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local _, input_pins, output_pins = reaper.TrackFX_GetIOSize(track, fx)
  local function read(is_output, count)
    local pins = {}
    for pin = 0, count - 1 do
      local low = reaper.TrackFX_GetPinMappings(track, fx, is_output, pin)
      pins[#pins + 1] = low
    end
    return pins
  end
  return {
    fx = fx_name(track, fx),
    input_pin_count = input_pins,
    output_pin_count = output_pins,
    inputs = read(0, input_pins),
    outputs = read(1, output_pins),
  }
end

handlers.set_fx_pins = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  return undo("set FX pins", function()
    for _, entry in ipairs(args.pins) do
      local pin = require_int(entry.pin, "pin", 0)
      local mask = require_int(entry.mask, "mask", 0)
      local is_output = entry.output and 1 or 0
      if not reaper.TrackFX_SetPinMappings(track, fx, is_output, pin, mask, 0) then
        error("REAPER rejected the pin mapping")
      end
    end
    return handlers.get_fx_pins({ track = args.track, fx = args.fx })
  end)
end

-- FX chain files ---------------------------------------------------------------------------------
-- REAPER has no function that writes a plugin preset file, so the state of plugins travels as an FX chain
-- (.RfxChain, the same text REAPER's own "Save FX chain" writes): one unit per plugin, with its state inside.
local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  if lines[#lines] == "" then lines[#lines] = nil end
  return lines
end

-- The lines of the track's <FXCHAIN ...> block: lines, first and last line index.
local function fxchain_block(lines)
  local first, depth
  for i, line in ipairs(lines) do
    if not first then
      if line:match("^%s*<FXCHAIN%s*$") then first, depth = i, 1 end
    elseif line:match("^%s*<") then
      depth = depth + 1
    elseif line:match("^%s*>%s*$") then
      depth = depth - 1
      if depth == 0 then return first, i end
    end
  end
  return nil, nil
end

-- Cuts chain lines into plugin units: each starts with a BYPASS line (at depth 0 of the block) and holds
-- the <VST/<JS/<AU/<CLAP block, FXID and WAK. FXID lines are dropped: REAPER makes new ones.
local function chain_units(lines)
  local units, current, depth = {}, nil, 0
  for _, line in ipairs(lines) do
    if depth == 0 and line:match("^%s*BYPASS%s") then
      current = { lines = {}, name = nil }
      units[#units + 1] = current
    end
    if current and not (depth == 0 and line:match("^%s*FXID%s")) then
      current.lines[#current.lines + 1] = line
      if not current.name then current.name = line:match('^%s*<[%u]+%s+"([^"]+)"') or line:match("^%s*<[%u]+%s+(%S+)") end
    end
    if line:match("^%s*<") then depth = depth + 1 elseif line:match("^%s*>%s*$") then depth = depth - 1 end
  end
  return units
end

local function wanted_units(units, selection)
  if selection == nil then return units end
  if type(selection) ~= "table" then error("fx must be a list of indexes or names") end
  local chosen = {}
  for _, ref in ipairs(selection) do
    if type(ref) == "number" then
      local unit = units[ref + 1]
      if not unit then error("fx index is out of range: " .. tostring(ref)) end
      chosen[#chosen + 1] = unit
    else
      local found
      for _, unit in ipairs(units) do
        if unit.name and unit.name:lower():find(tostring(ref):lower(), 1, true) then found = unit; break end
      end
      if not found then error("no FX matches '" .. tostring(ref) .. "'") end
      chosen[#chosen + 1] = found
    end
  end
  return chosen
end

local function track_chain(track)
  local ok, chunk = reaper.GetTrackStateChunk(track, "", false)
  if not ok then error("could not read the track state") end
  local lines = split_lines(chunk)
  local first, last = fxchain_block(lines)
  local inner = {}
  if first then for i = first + 1, last - 1 do inner[#inner + 1] = lines[i] end end
  return lines, first, last, chain_units(inner)
end

-- Writes the FX of a track (all, or those named in `fx`: indexes or name parts) to an .RfxChain file.
handlers.export_fx_chain = function(args)
  local track = resolve_track(args.track)
  if type(args.path) ~= "string" or args.path == "" then error("path must be a non-empty string") end
  local _, _, _, units = track_chain(track)
  if #units == 0 then error("the track has no FX") end
  local chosen = wanted_units(units, args.fx)
  local out = {}
  for _, unit in ipairs(chosen) do for _, line in ipairs(unit.lines) do out[#out + 1] = line end end
  local file, err = io.open(args.path, "wb")
  if not file then error("could not write " .. args.path .. ": " .. tostring(err)) end
  file:write(table.concat(out, "\n") .. "\n")
  file:close()
  local names = {}
  for _, unit in ipairs(chosen) do names[#names + 1] = unit.name end
  return { path = args.path, count = #chosen, plugins = names }
end

-- Puts the FX of an .RfxChain file on a track: mode append (default, after the existing FX) or replace.
handlers.import_fx_chain = function(args)
  local track = resolve_track(args.track)
  if type(args.path) ~= "string" or not file_exists(args.path) then error("path must be an existing .RfxChain file") end
  local mode = args.mode or "append"
  if mode ~= "append" and mode ~= "replace" then error("mode must be append or replace") end
  local file = io.open(args.path, "rb")
  local text = file:read("*a")
  file:close()
  local imported = chain_units(split_lines(text))
  if #imported == 0 then error("the file holds no FX (no BYPASS lines)") end
  local lines, first, last, current = track_chain(track)
  local block = { "<FXCHAIN", "SHOW 0", "LASTSEL 0", "DOCKED 0" }
  if mode == "append" then
    for _, unit in ipairs(current) do for _, line in ipairs(unit.lines) do block[#block + 1] = line end end
  end
  for _, unit in ipairs(imported) do for _, line in ipairs(unit.lines) do block[#block + 1] = line end end
  block[#block + 1] = ">"
  local result = {}
  if first then
    for i = 1, first - 1 do result[#result + 1] = lines[i] end
    for _, line in ipairs(block) do result[#result + 1] = line end
    for i = last + 1, #lines do result[#result + 1] = lines[i] end
  else  -- no FX chain yet: before the closing > of the track
    for i = 1, #lines - 1 do result[#result + 1] = lines[i] end
    for _, line in ipairs(block) do result[#result + 1] = line end
    result[#result + 1] = lines[#lines]
  end
  return undo("import FX chain", function()
    if not reaper.SetTrackStateChunk(track, table.concat(result, "\n") .. "\n", false) then
      error("REAPER refused the track state")
    end
    local names = {}
    for _, unit in ipairs(imported) do names[#names + 1] = unit.name end
    return { path = args.path, mode = mode, added = #imported, plugins = names, fx_count = reaper.TrackFX_GetCount(track) }
  end)
end

return handlers
