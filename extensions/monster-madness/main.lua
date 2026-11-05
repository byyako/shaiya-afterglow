-- Monster madness (docs/plans/ext-5-lua.md section 11.4).
local o = game.options

local allowed = {}
for _, id in ipairs(o.mobs) do allowed[id] = true end

-- The madness running: the monsters' ids, where, and when it ends. One at a time per map is not enforced; all of them count together.
local uids, killed, map, ends = {}, 0, nil, 0
local ending = nil

local function alive() return game.alive(uids) end

local function finish(why)
  if #uids == 0 then return end
  game.notice_map(map, "The madness is over: " .. killed .. " monsters fell" .. why)
  game.despawn(uids)
  uids, killed, ending = {}, 0, nil
end

-- Spawns `count` of `mob` around the player; the number made, or nil and the reason.
local function start(p, mob, count, minutes)
  if #o.mobs > 0 and not allowed[mob] then return nil, "That monster is not allowed." end
  minutes = math.min(minutes, o.max_minutes)
  local room = o.max_alive - alive()
  count = math.min(count, room)
  if count <= 0 then return nil, "Too many monsters alive already." end
  local x, _, z = p:position()
  local where = p:map()
  local made = 0
  while made < count do
    local batch = game.spawn(mob, math.min(50, count - made), where, x, z, { radius = 12, lifetime = minutes * 60 })
    if #batch == 0 then break end
    for _, uid in ipairs(batch) do uids[#uids + 1] = uid end
    made = made + #batch
  end
  if made == 0 then return nil, "Nothing could be placed there." end
  map = where
  game.notice_map(where, made .. " monsters appear! They stay for " .. minutes .. " minutes.")
  if ending then game.cancel(ending) end
  ending = game.after(minutes * 60 + 1, function() finish(" before the time ran out.") end)
  return made
end

game.command("madness", function(p, args)
  if args == "stop" then
    if #uids == 0 then return p:message("No madness is running.") end
    return finish(" before it was stopped.")
  end
  local mob, count, minutes = args:match("^(%d+)%s+(%d+)%s+(%d+)$")
  if not mob then return p:message("Usage: !madness <monster id> <count> <minutes> | !madness stop") end
  local made, why = start(p, tonumber(mob), tonumber(count), tonumber(minutes))
  if not made then p:message(why) end
end, { staff = 3, help = "!madness <monster id> <count> <minutes> | stop: a swarm around you" })

game.on("kill", function(p, mob)
  if mob.spawned ~= game.extension then return end
  killed = killed + 1
  if #uids > 0 and alive() == 0 then finish(".") end
end)

if o.item ~= "" then
  game.on_item({ o.item }, function(p)
    local made, why = start(p, o.item_mob, o.item_count, o.item_minutes)
    if not made then p:message(why) return false end
    return "use"
  end)
end
