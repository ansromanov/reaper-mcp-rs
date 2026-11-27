-- Track, send, mix-template and multi-output handlers.
-- Loaded by reaper_mcp_handlers.lua with the shared helpers and the handlers table.

local S, handlers = ...

local db_to_gain, describe_send, describe_track, fx_name, get_track_name, require_int, resolve_track, safe_filename, undo = S.db_to_gain, S.describe_send, S.describe_track, S.fx_name, S.get_track_name, S.require_int, S.resolve_track, S.safe_filename, S.undo

handlers.create_track = function(args)
  if type(args.name) ~= "string" or args.name:match("^%s*$") or #args.name > 255 then
    error("name must contain 1 to 255 bytes")
  end
  local index = reaper.CountTracks(0)
  reaper.Undo_BeginBlock2(0)
  reaper.InsertTrackAtIndex(index, true)
  local track = reaper.GetTrack(0, index)
  reaper.GetSetMediaTrackInfo_String(track, "P_NAME", args.name, true)
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  reaper.Undo_EndBlock2(0, "MCP: create track", -1)
  return { index = index, name = get_track_name(track), guid = reaper.GetTrackGUID(track) }
end

handlers.rename_track = function(args)
  if type(args.index) ~= "number" or args.index < 0 or args.index % 1 ~= 0 then
    error("index must be a non-negative integer")
  end
  if type(args.name) ~= "string" or args.name:match("^%s*$") or #args.name > 255 then
    error("name must contain 1 to 255 bytes")
  end
  local track = reaper.GetTrack(0, args.index)
  if not track then error("track index is out of range") end
  local old_name = get_track_name(track)
  reaper.Undo_BeginBlock2(0)
  reaper.GetSetMediaTrackInfo_String(track, "P_NAME", args.name, true)
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  reaper.Undo_EndBlock2(0, "MCP: rename track", -1)
  return { index = args.index, old_name = old_name, name = get_track_name(track) }
end

-- Channel routing and phase of a send; shared by create_send and set_send.
local function apply_send_routing(source, send, args)
  if args.source_channel ~= nil then
    reaper.SetTrackSendInfo_Value(source, 0, send, "I_SRCCHAN", require_int(args.source_channel, "source_channel", -1))
  end
  if args.dest_channel ~= nil then
    reaper.SetTrackSendInfo_Value(source, 0, send, "I_DSTCHAN", require_int(args.dest_channel, "dest_channel", 0))
  end
  if args.mono ~= nil then reaper.SetTrackSendInfo_Value(source, 0, send, "B_MONO", args.mono and 1 or 0) end
  if args.mode ~= nil then
    local mode = require_int(args.mode, "mode", 0)
    if mode ~= 0 and mode ~= 1 and mode ~= 3 then error("mode must be 0 (post-fader), 1 (pre-FX) or 3 (post-FX, pre-fader)") end
    reaper.SetTrackSendInfo_Value(source, 0, send, "I_SENDMODE", mode)
  end
  if args.phase_inverted ~= nil then
    reaper.SetTrackSendInfo_Value(source, 0, send, "B_PHASE", args.phase_inverted and 1 or 0)
  end
end

local function apply_track_fields(track, args)
  if args.name ~= nil then
    if type(args.name) ~= "string" or args.name:match("^%s*$") or #args.name > 255 then
      error("name must contain 1 to 255 bytes")
    end
    reaper.GetSetMediaTrackInfo_String(track, "P_NAME", args.name, true)
  end
  if args.volume_db ~= nil then
    reaper.SetMediaTrackInfo_Value(track, "D_VOL", db_to_gain(args.volume_db))
  end
  if args.pan ~= nil then reaper.SetMediaTrackInfo_Value(track, "D_PAN", args.pan) end
  if args.mute ~= nil then reaper.SetMediaTrackInfo_Value(track, "B_MUTE", args.mute and 1 or 0) end
  if args.solo ~= nil then reaper.SetMediaTrackInfo_Value(track, "I_SOLO", args.solo and 2 or 0) end
  if args.phase_inverted ~= nil then
    reaper.SetMediaTrackInfo_Value(track, "B_PHASE", args.phase_inverted and 1 or 0)
  end
  if args.master_send ~= nil then
    reaper.SetMediaTrackInfo_Value(track, "B_MAINSEND", args.master_send and 1 or 0)
  end
  if args.fx_enabled ~= nil then
    reaper.SetMediaTrackInfo_Value(track, "I_FXEN", args.fx_enabled and 1 or 0)
  end
  if args.channels ~= nil then
    local channels = math.floor(args.channels)
    if channels < 2 or channels > 128 or channels % 2 ~= 0 then
      error("channels must be an even number from 2 to 128")
    end
    reaper.SetMediaTrackInfo_Value(track, "I_NCHAN", channels)
  end
end

-- Export grouped, item-free track templates from the active project. Track state
-- chunks contain the complete FX, envelopes, folder depth, and send configuration.
local MIX_TEMPLATE_GROUPS = { "DRUMS", "BASS", "GUITARS", "VOCALS", "MIX BUS", "RETURNS" }

local function remove_item_chunks(chunk)
  local output, skipping, depth = {}, false, 0
  for line in (chunk .. "\n"):gmatch("(.-)\n") do
    local keep = not skipping
    if not skipping and line:match("^%s*<ITEM[%s>]?") then
      skipping = true
      depth = 1
      keep = false
    elseif skipping and line:match("^%s*<") then
      depth = depth + 1
      keep = false
    elseif skipping and line:match("^%s*>%s*$") then
      depth = depth - 1
      keep = false
      if depth == 0 then skipping = false end
    elseif skipping then
      keep = false
    end
    if keep then output[#output + 1] = line end
  end
  return table.concat(output, "\n") .. "\n"
end

local function direct_child_chunk(chunk, wanted_tag)
  local lines, depth, start = {}, 0, nil
  for line in (chunk .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  for i, line in ipairs(lines) do
    if line:match("^%s*>%s*$") then
      depth = depth - 1
      if start and depth == 1 then
        local block = {}
        for j = start, i do block[#block + 1] = lines[j] end
        return table.concat(block, "\n") .. "\n"
      end
    else
      local tag = line:match("^%s*<([%w_]+)")
      if tag then
        if depth == 1 and tag == wanted_tag then start = i end
        depth = depth + 1
      end
    end
  end
  return nil
end

local function remap_receives(chunk, local_track_indices)
  local output, depth, skipping, omitted = {}, 0, false, 0
  for line in (chunk .. "\n"):gmatch("(.-)\n") do
    local was_skipping = skipping
    local keep = not was_skipping
    if line:match("^%s*>%s*$") then
      depth = depth - 1
      if skipping and depth == 1 then skipping = false end
      keep = not was_skipping
    else
      local tag = line:match("^%s*<([%w_]+)")
      if tag then
        if depth == 1 and tag == "AUXRECV" then
          local prefix, old_index, suffix = line:match("^(%s*<AUXRECV%s+)(%d+)(.*)$")
          if not prefix then error("malformed AUXRECV chunk: missing source track index") end
          local local_index = local_track_indices[tonumber(old_index)]
          if local_index == nil then
            omitted = omitted + 1
            skipping = true
            keep = false
          else
            line = prefix .. tostring(local_index) .. suffix
          end
        end
        depth = depth + 1
      elseif skipping then
        keep = false
      end
    end
    if keep then output[#output + 1] = line end
  end
  return table.concat(output, "\n") .. "\n", omitted
end

handlers.export_mix_template = function(args)
  if type(args.directory) ~= "string" or args.directory:match("^%s*$") then
    error("directory must be a non-empty path")
  end
  if args.include_master_chain ~= nil and type(args.include_master_chain) ~= "boolean" then
    error("include_master_chain must be a boolean")
  end
  local requested, group_order = {}, {}
  if args.groups == nil then
    for _, group in ipairs(MIX_TEMPLATE_GROUPS) do
      requested[group] = true
      group_order[#group_order + 1] = group
    end
  elseif type(args.groups) == "table" and #args.groups > 0 then
    for _, group in ipairs(args.groups) do
      if type(group) ~= "string" or group:match("^%s*$") then error("groups must contain non-empty names") end
      local normalized = group:upper()
      if not requested[normalized] then group_order[#group_order + 1] = normalized end
      requested[normalized] = true
    end
  else
    error("groups must be a non-empty array of group names")
  end

  local tracks, track_count = {}, reaper.CountTracks(0)
  for i = 0, track_count - 1 do
    local track = reaper.GetTrack(0, i)
    local _, chunk = reaper.GetTrackStateChunk(track, "", false)
    if not chunk or chunk == "" then error("could not read track state for index " .. tostring(i)) end
    tracks[i + 1] = {
      name = get_track_name(track),
      folder_depth = reaper.GetMediaTrackInfo_Value(track, "I_FOLDERDEPTH"),
      chunk = remove_item_chunks(chunk),
    }
  end

  local project_name = reaper.GetProjectName(0, "")
  project_name = project_name:gsub("%.[Rr][Pp][Pp]$", "")
  project_name = safe_filename(project_name)
  reaper.RecursiveCreateDirectory(args.directory, 0)
  local results = {}
  for _, group_name in ipairs(group_order) do
    local included, depth, folder_roots = {}, 0, {}
    for i, track in ipairs(tracks) do
      local match = track.name:upper() == group_name
      for root_level = #folder_roots, 1, -1 do
        if depth <= folder_roots[root_level] then table.remove(folder_roots, root_level) end
      end
      local is_descendant = #folder_roots > 0 and depth > folder_roots[#folder_roots]
      if match or is_descendant then included[#included + 1] = i end
      if match and track.folder_depth > 0 then folder_roots[#folder_roots + 1] = depth end
      depth = depth + track.folder_depth
      if depth < 0 then depth = 0 end
    end
    if #included > 0 then
      local group_file = safe_filename(group_name:lower():gsub("%s+", "-"))
      local path = args.directory .. "/" .. project_name .. "-" .. group_file .. ".RTrackTemplate"
      local file, err = io.open(path, "wb")
      if not file then error("could not open template for writing: " .. tostring(err)) end
      local local_track_indices = {}
      for local_index, source_index in ipairs(included) do
        local_track_indices[source_index - 1] = local_index - 1
      end
      local omitted_receives = 0
      for _, index in ipairs(included) do
        local chunk, omitted = remap_receives(tracks[index].chunk, local_track_indices)
        file:write(chunk)
        omitted_receives = omitted_receives + omitted
      end
      file:close()
      results[#results + 1] = {
        group = group_name,
        path = path,
        track_count = #included,
        omitted_external_receives = omitted_receives,
      }
    end
  end

  local master_chain = nil
  if args.include_master_chain ~= false then
    local master = reaper.GetMasterTrack(0)
    local ok, chunk = reaper.GetTrackStateChunk(master, "", false)
    if not ok or not chunk then error("could not read master track state") end
    -- REAPER's master track uses MASTERFXLIST in its state chunk, while the
    -- portable .RfxChain file format expects an FXCHAIN root.
    local fxchain = direct_child_chunk(chunk, "MASTERFXLIST")
    if fxchain then
      fxchain = fxchain:gsub("^([ \t]*)<MASTERFXLIST", "%1<FXCHAIN", 1)
    else
      fxchain = direct_child_chunk(chunk, "FXCHAIN")
    end
    if fxchain then
      local path = args.directory .. "/master.RfxChain"
      local file, err = io.open(path, "wb")
      if not file then error("could not open master FX chain for writing: " .. tostring(err)) end
      file:write(fxchain)
      file:close()
      local fx_count = 0
      for _ in fxchain:gmatch("\n%s*FXID%s+") do fx_count = fx_count + 1 end
      master_chain = { path = path, fx_count = fx_count }
    else
      master_chain = { path = nil, fx_count = 0 }
    end
  end
  return {
    project_name = project_name,
    directory = args.directory,
    templates = results,
    exported_groups = #results,
    master_chain = master_chain,
    warning = "Receives from tracks outside a group are omitted. Internal receives are remapped to template-local track indexes. The separate master FX chain contains only static FX state.",
  }
end

handlers.get_track_state_chunk = function(args)
  local track, index = resolve_track(args.track)
  local ok, chunk = reaper.GetTrackStateChunk(track, "", false)
  if not ok or not chunk or chunk == "" then
    error("could not read track state chunk for index " .. tostring(index))
  end
  return { track = index, chunk = chunk }
end

handlers.set_track_state_chunk = function(args)
  local track, index = resolve_track(args.track)
  local valid_chunk = type(args.chunk) == "string"
      and (args.chunk:match("^%s*<TRACK[%s>]" ) or args.chunk:match("^%s*<MASTERTRACK[%s>]"))
  if not valid_chunk then
    error("chunk must contain a REAPER <TRACK> or <MASTERTRACK> state chunk")
  end
  local old_fx_count = reaper.TrackFX_GetCount(track)
  reaper.Undo_BeginBlock2(0)
  local ok = reaper.SetTrackStateChunk(track, args.chunk, true)
  reaper.Undo_EndBlock2(0, "MCP: apply track FX template", -1)
  if not ok then error("REAPER rejected the track state chunk for index " .. tostring(index)) end
  if args.close_fx_windows then
    local first_fx = args.preserve_existing_fx_windows and old_fx_count or 0
    if first_fx == 0 then reaper.TrackFX_Show(track, 0, 0) end
    for fx = first_fx, reaper.TrackFX_GetCount(track) - 1 do
      reaper.TrackFX_Show(track, fx, 2)
    end
  end
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  reaper.MarkProjectDirty(0)
  return { track = index, applied = true }
end

handlers.get_track = function(args)
  local track, index = resolve_track(args.track)
  return describe_track(track, index)
end

handlers.list_tracks = function()
  local tracks = {}
  for index = 0, reaper.CountTracks(0) - 1 do
    tracks[#tracks + 1] = describe_track(reaper.GetTrack(0, index), index)
  end
  return tracks
end

-- Moves one track so it sits before `before_index` (0-based; CountTracks() = to the end).
-- Does not touch folder state: build the order first, then set folder depths.
handlers.move_track = function(args)
  local count = reaper.CountTracks(0)
  if type(args.index) ~= "number" or args.index < 0 or args.index >= count or args.index % 1 ~= 0 then
    error("index is out of range")
  end
  if type(args.before_index) ~= "number" or args.before_index < 0 or args.before_index > count or args.before_index % 1 ~= 0 then
    error("before_index must be between 0 and the track count")
  end
  local track = reaper.GetTrack(0, args.index)
  local guid = reaper.GetTrackGUID(track)
  reaper.Undo_BeginBlock2(0)
  reaper.PreventUIRefresh(1)
  reaper.Main_OnCommand(40297, 0) -- unselect all tracks
  reaper.SetTrackSelected(track, true)
  reaper.ReorderSelectedTracks(args.before_index, 0)
  reaper.Main_OnCommand(40297, 0)
  reaper.PreventUIRefresh(-1)
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  reaper.Undo_EndBlock2(0, "MCP: move track", -1)
  for i = 0, reaper.CountTracks(0) - 1 do
    if reaper.GetTrackGUID(reaper.GetTrack(0, i)) == guid then return { index = i } end
  end
  error("moved track not found")
end

-- Sets I_FOLDERDEPTH: 1 starts a folder, 0 is a normal track, -1..-n closes that many folder levels.
handlers.set_track_folder_depth = function(args)
  local count = reaper.CountTracks(0)
  if type(args.index) ~= "number" or args.index < 0 or args.index >= count or args.index % 1 ~= 0 then
    error("index is out of range")
  end
  if type(args.depth) ~= "number" or args.depth % 1 ~= 0 or args.depth < -16 or args.depth > 1 then
    error("depth must be an integer from -16 to 1")
  end
  local track = reaper.GetTrack(0, args.index)
  reaper.Undo_BeginBlock2(0)
  reaper.SetMediaTrackInfo_Value(track, "I_FOLDERDEPTH", args.depth)
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  reaper.Undo_EndBlock2(0, "MCP: set folder depth", -1)
  return { index = args.index, depth = reaper.GetMediaTrackInfo_Value(track, "I_FOLDERDEPTH") }
end

handlers.set_track = function(args)
  local track, index = resolve_track(args.track)
  return undo("set track", function()
    apply_track_fields(track, args)
    return describe_track(track, index)
  end)
end

-- One call that answers "what can change the level and sound of this track": FX-chain enable state, FX chain with pins,
-- fader, envelopes, sends, parent, and a note when something acts after the plugins.
handlers.describe_track = function(args)
  local track, index = resolve_track(args.track)
  local info = describe_track(track, index)
  local chain = {}
  for fx = 0, reaper.TrackFX_GetCount(track) - 1 do
    local _, inputs, outputs = reaper.TrackFX_GetIOSize(track, fx)
    local in_pins, out_pins = {}, {}
    for pin = 0, inputs - 1 do in_pins[#in_pins + 1] = (reaper.TrackFX_GetPinMappings(track, fx, 0, pin)) end
    for pin = 0, outputs - 1 do out_pins[#out_pins + 1] = (reaper.TrackFX_GetPinMappings(track, fx, 1, pin)) end
    chain[#chain + 1] = {
      index = fx,
      name = fx_name(track, fx),
      enabled = reaper.TrackFX_GetEnabled(track, fx),
      input_pins = in_pins,
      output_pins = out_pins,
    }
  end
  local report = handlers.list_track_envelopes({ track = index })
  local sends = handlers.list_sends({ track = index })
  local notes = {}
  local gain_low, gain_high = info.volume_db, info.volume_db
  for _, env in ipairs(report.envelopes) do
    if env.name == "Volume" and env.active and env.points > 0 then
      gain_low, gain_high = info.volume_db + env.min, info.volume_db + env.max
      notes[#notes + 1] = string.format(
        "an active Volume envelope runs after the FX chain: %.1f to %.1f dB on top of the fader, so a limiter ceiling on this track is shifted by that amount (post-FX gain %.1f to %.1f dB)",
        env.min, env.max, gain_low, gain_high)
    elseif env.active and env.points > 0 then
      notes[#notes + 1] = string.format("active %s envelope (%d points)", env.name, env.points)
    end
  end
  if #report.fx_envelopes > 0 then
    notes[#notes + 1] = string.format("%d FX parameter envelope(s): parameter values change over time", #report.fx_envelopes)
  end
  if info.volume_db > 0.05 then
    notes[#notes + 1] = string.format("fader is +%.1f dB after the FX chain", info.volume_db)
  end
  info.fx_chain = chain
  info.envelopes = report.envelopes
  info.fx_envelopes = report.fx_envelopes
  info.sends = sends.sends
  info.receives = sends.receives
  info.post_fx_gain_db = { minimum = gain_low, maximum = gain_high }
  info.notes = notes
  return info
end

handlers.list_sends = function(args)
  local track = resolve_track(args.track)
  local sends, receives = {}, {}
  for i = 0, reaper.GetTrackNumSends(track, 0) - 1 do sends[#sends + 1] = describe_send(track, 0, i) end
  for i = 0, reaper.GetTrackNumSends(track, -1) - 1 do receives[#receives + 1] = describe_send(track, -1, i) end
  return { sends = sends, receives = receives }
end

handlers.create_send = function(args)
  local source = resolve_track(args.source, "source")
  local destination = resolve_track(args.destination, "destination")
  return undo("create send", function()
    local send = reaper.CreateTrackSend(source, destination)
    if send < 0 then error("REAPER could not create the send") end
    if args.volume_db ~= nil then
      reaper.SetTrackSendInfo_Value(source, 0, send, "D_VOL", db_to_gain(args.volume_db))
    end
    if args.pan ~= nil then reaper.SetTrackSendInfo_Value(source, 0, send, "D_PAN", args.pan) end
    apply_send_routing(source, send, args)
    return describe_send(source, 0, send)
  end)
end

handlers.set_send = function(args)
  local source = resolve_track(args.source, "source")
  local send = require_int(args.send_index, "send_index", 0)
  if send >= reaper.GetTrackNumSends(source, 0) then error("send_index is out of range") end
  return undo("set send", function()
    if args.volume_db ~= nil then
      reaper.SetTrackSendInfo_Value(source, 0, send, "D_VOL", db_to_gain(args.volume_db))
    end
    if args.pan ~= nil then reaper.SetTrackSendInfo_Value(source, 0, send, "D_PAN", args.pan) end
    if args.mute ~= nil then reaper.SetTrackSendInfo_Value(source, 0, send, "B_MUTE", args.mute and 1 or 0) end
    apply_send_routing(source, send, args)
    return describe_send(source, 0, send)
  end)
end

-- Removes one send. The send indexes of that track after it shift down by one.
handlers.remove_send = function(args)
  local source = resolve_track(args.source, "source")
  local send = require_int(args.send_index, "send_index", 0)
  if send >= reaper.GetTrackNumSends(source, 0) then error("send_index is out of range") end
  return undo("remove send", function()
    if not reaper.RemoveTrackSend(source, 0, send) then error("REAPER could not remove the send") end
    return handlers.list_sends({ track = args.source })
  end)
end

-- Multi-out instruments (Kontakt, drum samplers) ---------------------------------------------
-- An instrument such as Kontakt can play every part of a kit on its own output pair. REAPER only
-- passes those outputs on when the instrument track has enough channels and other tracks receive
-- the pairs through sends.

local function instrument_output_channels(track)
  local most = 2
  for fx = 0, reaper.TrackFX_GetCount(track) - 1 do
    local _, _, outputs = reaper.TrackFX_GetIOSize(track, fx)
    if outputs and outputs > most then most = outputs end
  end
  if most % 2 == 1 then most = most + 1 end
  return math.min(most, 128)
end

local function ensure_channels(track, needed)
  local current = math.floor(reaper.GetMediaTrackInfo_Value(track, "I_NCHAN"))
  if needed % 2 == 1 then needed = needed + 1 end
  if needed > current then reaper.SetMediaTrackInfo_Value(track, "I_NCHAN", math.min(needed, 128)) end
  return current
end

-- outputs = [{ name, source_channel (0 = outputs 1/2, 2 = 3/4 ...), mono?, volume_db?, pan? }].
-- The new tracks go right after the instrument, inside an optional new folder track.
handlers.create_multiout_tracks = function(args)
  local instrument, index = resolve_track(args.track)
  if index < 0 then error("the master track cannot be an instrument") end
  if type(args.outputs) ~= "table" or #args.outputs == 0 then error("outputs must be a non-empty list") end
  for i, output in ipairs(args.outputs) do
    if type(output.name) ~= "string" or output.name:match("^%s*$") then error("outputs[" .. i .. "].name is missing") end
    require_int(output.source_channel, "outputs[" .. i .. "].source_channel", 0)
  end
  return undo("create multi-out tracks", function()
    local highest = 0
    for _, output in ipairs(args.outputs) do
      highest = math.max(highest, output.source_channel + (output.mono and 1 or 2))
    end
    local before_channels = ensure_channels(instrument, highest)
    local closing = reaper.GetMediaTrackInfo_Value(instrument, "I_FOLDERDEPTH")
    if closing < 0 then reaper.SetMediaTrackInfo_Value(instrument, "I_FOLDERDEPTH", 0) end
    local at = index + 1
    local created = {}
    local folder
    if args.folder then
      reaper.InsertTrackAtIndex(at, true)
      folder = reaper.GetTrack(0, at)
      reaper.GetSetMediaTrackInfo_String(folder, "P_NAME", args.folder, true)
      reaper.SetMediaTrackInfo_Value(folder, "I_FOLDERDEPTH", 1)
      at = at + 1
    end
    local last
    for _, output in ipairs(args.outputs) do
      reaper.InsertTrackAtIndex(at, true)
      local track = reaper.GetTrack(0, at)
      reaper.GetSetMediaTrackInfo_String(track, "P_NAME", output.name, true)
      reaper.SetMediaTrackInfo_Value(track, "I_FOLDERDEPTH", 0)
      if output.volume_db ~= nil then reaper.SetMediaTrackInfo_Value(track, "D_VOL", db_to_gain(output.volume_db)) end
      if output.pan ~= nil then reaper.SetMediaTrackInfo_Value(track, "D_PAN", output.pan) end
      local send = reaper.CreateTrackSend(instrument, track)
      if send < 0 then error("REAPER could not create the send to " .. output.name) end
      apply_send_routing(instrument, send, {
        source_channel = output.mono and (1024 + output.source_channel) or output.source_channel,
        dest_channel = 0, mode = args.send_mode or 0,
      })
      created[#created + 1] = { index = at, name = output.name, source_channel = output.source_channel, mono = output.mono == true }
      last = track
      at = at + 1
    end
    -- close the new folder, then whatever folders the instrument used to close
    local tail_depth = (folder and -1 or 0) + (closing < 0 and closing or 0)
    reaper.SetMediaTrackInfo_Value(last, "I_FOLDERDEPTH", tail_depth)
    if args.instrument_to_parent == false then reaper.SetMediaTrackInfo_Value(instrument, "B_MAINSEND", 0) end
    return {
      instrument = get_track_name(instrument),
      channels = math.floor(reaper.GetMediaTrackInfo_Value(instrument, "I_NCHAN")),
      channels_before = before_channels,
      folder = folder and { index = index + 1, name = args.folder } or nil,
      tracks = created,
    }
  end)
end

-- Renders every output pair of an instrument to its own file so the caller can see which pairs
-- carry sound (the MCP server measures the files). Temporary receive tracks are removed again and
-- the track's channel count and mute state are restored; nothing else in the project changes.
handlers.probe_instrument_outputs = function(args)
  local instrument, index = resolve_track(args.track)
  if index < 0 then error("the master track cannot be an instrument") end
  if type(args.directory) ~= "string" or args.directory == "" then error("directory must be a non-empty string") end
  local channels = args.channels and require_int(args.channels, "channels", 2) or instrument_output_channels(instrument)
  local before_channels = math.floor(reaper.GetMediaTrackInfo_Value(instrument, "I_NCHAN"))
  local before_mute = reaper.GetMediaTrackInfo_Value(instrument, "B_MUTE")
  local before_sends = reaper.GetTrackNumSends(instrument, 0)
  local probes = {}
  reaper.PreventUIRefresh(1)
  local ok, result = pcall(function()
    ensure_channels(instrument, channels)
    reaper.SetMediaTrackInfo_Value(instrument, "B_MUTE", 0)
    for pair = 0, channels - 2, 2 do
      local at = reaper.CountTracks(0)
      reaper.InsertTrackAtIndex(at, false)
      local track = reaper.GetTrack(0, at)
      reaper.GetSetMediaTrackInfo_String(track, "P_NAME", string.format("__probe_out_%02d-%02d", pair + 1, pair + 2), true)
      local send = reaper.CreateTrackSend(instrument, track)
      apply_send_routing(instrument, send, { source_channel = pair, dest_channel = 0, mode = 3 })
      probes[#probes + 1] = { track = track, pair = pair }
    end
    local indexes = {}
    for _, probe in ipairs(probes) do
      indexes[#indexes + 1] = math.floor(reaper.GetMediaTrackInfo_Value(probe.track, "IP_TRACKNUMBER")) - 1
    end
    return handlers.render_stems({
      directory = args.directory, tracks = indexes, format = "float32",
      start_seconds = args.start_seconds, end_seconds = args.end_seconds, fresh = false,
    })
  end)
  for i = #probes, 1, -1 do reaper.DeleteTrack(probes[i].track) end
  for send = reaper.GetTrackNumSends(instrument, 0) - 1, before_sends, -1 do reaper.RemoveTrackSend(instrument, 0, send) end
  reaper.SetMediaTrackInfo_Value(instrument, "B_MUTE", before_mute)
  reaper.SetMediaTrackInfo_Value(instrument, "I_NCHAN", before_channels)
  reaper.PreventUIRefresh(-1)
  if not ok then error(result, 0) end
  local pairs_out = {}
  for i, probe in ipairs(probes) do
    pairs_out[i] = { source_channel = probe.pair, outputs = string.format("%d/%d", probe.pair + 1, probe.pair + 2),
                     file = result.files[i] and result.files[i].path or nil }
  end
  return { instrument = get_track_name(instrument), channels = channels, pairs = pairs_out,
           elapsed_seconds = result.elapsed_seconds, warnings = result.warnings }
end

-- ---------------------------------------------------------------------------
-- Deleting, media files, MIDI, envelopes, snapshots, plugins, variants, batches.
-- ---------------------------------------------------------------------------

-- Deletes a track; with_children also deletes everything inside a folder track. The master track
-- cannot be deleted. Without with_children the children of a folder stay and move up one level.
handlers.delete_track = function(args)
  local track, index = resolve_track(args.track)
  if index == -1 then error("the master track cannot be deleted") end
  return undo("delete track", function()
    local victims = { index }
    if args.with_children and reaper.GetMediaTrackInfo_Value(track, "I_FOLDERDEPTH") == 1 then
      local level = 0
      for i = index, reaper.CountTracks(0) - 1 do
        level = level + reaper.GetMediaTrackInfo_Value(reaper.GetTrack(0, i), "I_FOLDERDEPTH")
        if i > index then victims[#victims + 1] = i end
        if level <= 0 then break end
      end
    end
    table.sort(victims, function(a, b) return a > b end)
    local names = {}
    for _, i in ipairs(victims) do
      local victim = reaper.GetTrack(0, i)
      table.insert(names, 1, get_track_name(victim)) -- keep the project order
      reaper.DeleteTrack(victim)
    end
    return { deleted = names, track_count = reaper.CountTracks(0) }
  end)
end
