-- Command handlers for the reaper-mcp-rs bridge. Loaded by reaper_mcp_bridge.lua, which passes
-- the directory of this file, and reloaded by the `reload` command, so edits apply without
-- restarting REAPER. The handlers live in handlers/: `support.lua` holds the helpers several
-- of them share, the other files register the handlers of one area each.

local dir = ...
local handlers = {}
local support = dofile(dir .. "/handlers/support.lua")
for _, area in ipairs({ "project", "tracks", "items", "fx", "envelopes", "render" }) do
  assert(loadfile(dir .. "/handlers/" .. area .. ".lua"))(support, handlers)
end

return handlers
