-- Welcome notice, advert banner and chat commands (docs/plans/ext-5-lua.md section 11.2).
local o = game.options

local function greeting(p)
  return (o.text:gsub("{name}", p.name))
end

game.on("login", function(p)
  local text = greeting(p)
  p:message(text)
  p:notice(text)
end)

-- The adverts go out in turn, one every advert_minutes.
local next_advert = 1
if #o.adverts > 0 then
  game.every(o.advert_minutes * 60, function()
    local text = o.adverts[next_advert]
    next_advert = next_advert % #o.adverts + 1
    if o.advert_kind == "banner" then game.banner(text) else game.notice_all(text) end
  end)
end

game.command("help", function(p)
  local level = p:staff()
  local parts = {}
  for _, c in ipairs(game.commands()) do
    if c.staff == 0 or (level > 0 and level <= c.staff) then
      parts[#parts + 1] = "!" .. c.word .. (c.help ~= "" and (" - " .. c.help) or "")
    end
  end
  p:message("Commands: " .. table.concat(parts, "; "))
end, { help = "lists the commands you may use" })

game.command("online", function(p)
  p:message(game.online_count() .. " players online")
end, { help = "how many players are online" })

game.command("time", function(p)
  p:message("Server time " .. os.date("%H:%M"))
end, { help = "the server's local time" })
