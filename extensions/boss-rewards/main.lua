-- Boss and kill rewards (docs/plans/ext-5-lua.md section 11.3).
local o = game.options

-- "25/1" as 25, 1; nothing for an empty or wrong text.
local function pair(text)
  local a, b = text:match("^(%d+)/(%d+)$")
  if a then return tonumber(a), tonumber(b) end
end

local item_type, item_id = pair(o.item)
local buff_skill, buff_level = pair(o.buff)
local pvp_type, pvp_id = pair(o.pvp_item)

local function names(players)
  local list = {}
  for _, q in ipairs(players) do list[#list + 1] = q.name end
  return table.concat(list, ", ")
end

game.on("boss_kill", function(p, mob, near)
  for _, q in ipairs(near) do
    if item_type then q:give_item(item_type, item_id, o.item_count) end
    if buff_skill then q:add_buff(buff_skill, buff_level) end
    if o.gold > 0 then q:give_gold(o.gold) end
  end
  if o.announce then
    local boss = game.mob_name(mob.id) or ("boss " .. mob.id)
    game.notice_all(names(near) .. " defeated " .. boss .. "!")
  end
end)

game.on("pvp_kill", function(killer, victim, last_hit, near)
  if next(o.pvp_maps) ~= nil and not o.pvp_maps[killer:map()] then return end
  for _, q in ipairs(near) do
    if pvp_type then q:give_item(pvp_type, pvp_id, 1) end
    if o.pvp_gold > 0 then q:give_gold(o.pvp_gold) end
  end
end)
