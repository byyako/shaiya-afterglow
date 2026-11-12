-- Kill streaks and bounties (docs/plans/ext-5-lua.md section 11.1).
--
-- A streak is the player kills in a row without dying. From `bounty_from` kills the server puts a bounty on the streaker that grows with
-- the streak; players add gold to a bounty with !bounty. Whoever kills the player takes all of it. Stored: per character `streak`,
-- `best`, `last_victim`, `last_at`; the extension's own `bounty.<character id>` (the gold players added).
local o = game.options

local function counts(map)
  return next(o.maps) == nil or o.maps[map]
end

-- The gold the server itself puts on a player with this streak.
local function server_bounty(streak)
  if streak < o.bounty_from then return 0 end
  return math.min((streak - o.bounty_from + 1) * o.bounty_gold, o.bounty_cap)
end

-- The gold players added.
local function placed(id) return game.get("bounty." .. id) or 0 end

local function announced(n)
  for _, at in ipairs(o.announce_at) do if at == n then return true end end
  return false
end

game.on("pvp_kill", function(killer, victim)
  if not counts(killer:map()) then return end
  if killer:get("last_victim") == victim.id and game.now() - (killer:get("last_at") or 0) < o.repeat_secs then return end
  killer:set("last_victim", victim.id)
  killer:set("last_at", game.now())
  local streak = (killer:get("streak") or 0) + 1
  killer:set("streak", streak)
  if streak > (killer:get("best") or 0) then killer:set("best", streak) end
  if announced(streak) then
    game.notice_all(killer.name .. " has killed " .. streak .. " players in a row! Bounty: " .. (server_bounty(streak) + placed(killer.id)) .. " gold")
  end
end)

game.on("death", function(victim, by, killer, pvp)
  local streak = victim:get("streak") or 0
  local bounty = server_bounty(streak) + placed(victim.id)
  victim:set("streak", nil)
  if not pvp or bounty == 0 or not counts(victim:map()) then return end
  game.set("bounty." .. victim.id, nil)
  killer:give_gold(bounty, function(given)
    game.log(killer.name .. " took " .. given .. " gold for " .. victim.name)
  end)
  game.notice_all(killer.name .. " ended " .. victim.name .. "'s streak of " .. streak .. " and took the bounty of " .. bounty .. " gold")
end)

game.command("streak", function(p)
  local streak = p:get("streak") or 0
  p:message("Streak " .. streak .. ", best " .. (p:get("best") or 0) .. ", bounty on you " .. (server_bounty(streak) + placed(p.id)) .. " gold")
end, { help = "your kill streak and the bounty on you" })

game.command("bounty", function(p, args)
  local name, gold = args:match("^(%S+)%s+(%d+)$")
  gold = tonumber(gold)
  local target = name and game.find(name)
  if not target or not gold then return p:message("Usage: !bounty <name online> <gold>") end
  if target.id == p.id then return p:message("Not on yourself.") end
  if gold < o.player_bounty_min then return p:message("At least " .. o.player_bounty_min .. " gold.") end
  if gold > 2147483647 then return p:message("Too much gold.") end
  p:take_gold(gold, function(ok)
    if not ok then return p:message("You do not have " .. gold .. " gold.") end
    game.set("bounty." .. target.id, placed(target.id) + gold)
    game.notice_all(p.name .. " put " .. gold .. " gold on " .. target.name .. "'s head (" .. placed(target.id) .. " in all)")
  end)
end, { help = "!bounty <name> <gold>: add gold to a player's bounty" })

game.command("bounties", function(p)
  local list = {}
  for _, q in ipairs(game.online()) do
    local b = server_bounty(q:get("streak") or 0) + placed(q.id)
    if b > 0 then list[#list + 1] = { q.name, b } end
  end
  table.sort(list, function(a, b) return a[2] > b[2] end)
  if #list == 0 then return p:message("No bounties.") end
  for i = 1, math.min(5, #list) do p:message(list[i][1] .. ": " .. list[i][2] .. " gold") end
end, { help = "the largest bounties online" })
