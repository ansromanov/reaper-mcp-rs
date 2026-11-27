-- reaper-mcp-rs bridge loader. Polls request.json in the IPC directory, runs the
-- matching handler from reaper_mcp_handlers.lua and writes response.json.
--
-- Starting this script again supersedes the running copy: each copy writes a
-- generation id and a copy that sees a different id stops itself. The
-- `reload` command re-reads the handlers module without restarting.
local script_path = debug.getinfo(1, "S").source:sub(2)
local script_dir = script_path:match("^(.*)[/\\]") or "."
local json = dofile(script_dir .. "/vendor/json.lua")
local separator = package.config:sub(1, 1)
local ipc_dir = os.getenv("REAPER_MCP_DIR") or (reaper.GetResourcePath() .. separator .. "reaper-mcp-bridge")
local handlers_path = script_dir .. "/reaper_mcp_handlers.lua"
local generation_path = ipc_dir .. separator .. "generation"
local request_path = ipc_dir .. separator .. "request.json"
local response_path = ipc_dir .. separator .. "response.json"
local heartbeat_path = ipc_dir .. separator .. "heartbeat"
local last_heartbeat = 0
local last_generation_check = 0
local handlers

reaper.RecursiveCreateDirectory(ipc_dir, 0)
local my_generation = string.format("%d-%d", os.time(), math.floor(reaper.time_precise() * 1000))

local function read_text(path)
  local file = io.open(path, "rb")
  if not file then return nil end
  local contents = file:read("*a")
  file:close()
  return contents
end

local function write_text(path, text)
  local file = io.open(path, "wb")
  if not file then return false end
  file:write(text)
  file:close()
  return true
end

local function load_handlers()
  local loaded = assert(loadfile(handlers_path))(script_dir)
  if type(loaded) ~= "table" then error("handlers module did not return a table") end
  handlers = loaded
end

local function read_request()
  local contents = read_text(request_path)
  if not contents then return nil end
  os.remove(request_path)
  local ok, request = pcall(json.decode, contents)
  if not ok or type(request) ~= "table" then
    return { id = "invalid", command = "", args = {}, decode_error = tostring(request) }
  end
  return request
end

local function write_response(response)
  local temporary = response_path .. ".tmp"
  if not write_text(temporary, json.encode(response)) then return false end
  os.remove(response_path)
  return os.rename(temporary, response_path)
end

local function write_heartbeat()
  local temporary = heartbeat_path .. ".tmp"
  if write_text(temporary, tostring(os.time())) then
    os.remove(heartbeat_path)
    os.rename(temporary, heartbeat_path)
  end
  last_heartbeat = reaper.time_precise()
end

local function run_request(request)
  local response = { id = request.id, ok = false }
  if request.decode_error then
    response.error = "invalid JSON: " .. request.decode_error
    return response
  end
  if request.command == "reload" then
    local ok, err = pcall(load_handlers)
    if ok then
      response.ok = true
      response.result = { reloaded = true, generation = my_generation }
    else
      response.error = "reload failed: " .. tostring(err)
    end
    return response
  end
  local handler = handlers[request.command]
  if not handler then
    response.error = "unknown command: " .. tostring(request.command)
    return response
  end
  local ok, result = pcall(handler, request.args or {})
  if ok then
    response.ok = true
    response.result = result
  else
    response.error = tostring(result)
  end
  return response
end

local function superseded()
  local now = reaper.time_precise()
  if now - last_generation_check < 0.5 then return false end
  last_generation_check = now
  local current = read_text(generation_path)
  return current ~= nil and current ~= my_generation
end

local function loop()
  if superseded() then return end
  if reaper.time_precise() - last_heartbeat >= 1 then write_heartbeat() end

  local request = read_request()
  if request then
    write_response(run_request(request))
    -- Long commands (project load, render) stall the loop; refresh at once.
    write_heartbeat()
  end
  reaper.defer(loop)
end

load_handlers()
write_text(generation_path, my_generation)
loop()
