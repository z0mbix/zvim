local channel = ...
local pending = false
local previous
local function color(value)
  if type(value) == "string" then
    value = vim.api.nvim_get_color_by_name(value)
  end
  if type(value) == "number" and value >= 0 and value <= 0xffffff then
    return value
  end
  return vim.NIL
end
local function publish()
  pending = false
  local normal = vim.api.nvim_get_hl(0, { name = "Normal", link = false })
  local cursor = vim.api.nvim_get_hl(0, { name = "Cursor", link = false })
  local visual = vim.api.nvim_get_hl(0, { name = "Visual", link = false })
  local fg, bg = normal.fg, normal.bg
  if normal.reverse then fg, bg = bg, fg end
  local function pair(hl)
    local f, b = hl.fg, hl.bg
    if hl.reverse then f, b = b or bg, f or fg end
    return color(f), color(b)
  end
  local cursor_fg, cursor_bg = pair(cursor)
  local selection_fg, selection_bg = pair(visual)
  local values = { color(fg), color(bg), cursor_fg, cursor_bg, selection_fg, selection_bg }
  -- Base46 can reload highlights without reloading its generated terminal globals.
  local base46 = package.loaded.base46
  local palette
  if base46 and package.loaded.nvconfig and not vim.g.colors_name then
    local ok, theme = pcall(base46.get_theme_tb, "base_16")
    if ok and type(theme) == "table" then palette = theme end
  end
  local bases = { "base01", "base08", "base0B", "base0A", "base0D", "base0E", "base0C", "base05",
    "base03", "base08", "base0B", "base0A", "base0D", "base0E", "base0C", "base07" }
  for i = 0, 15 do
    values[#values + 1] = color(palette and palette[bases[i + 1]] or vim.g["terminal_color_" .. i])
  end
  if not vim.deep_equal(previous, values) then
    previous = values
    vim.rpcnotify(channel, "zvim_terminal_theme", values)
  end
end
_G.__zvim_terminal_theme_refresh = function()
  if pending then return end
  pending = true
  vim.defer_fn(publish, 30)
end
local group = vim.api.nvim_create_augroup("ZvimTerminalTheme", { clear = true })
vim.api.nvim_create_autocmd({ "ColorScheme", "VimEnter" }, {
  group = group, callback = _G.__zvim_terminal_theme_refresh,
})
vim.api.nvim_create_autocmd("OptionSet", {
  group = group, pattern = "background", callback = _G.__zvim_terminal_theme_refresh,
})
vim.api.nvim_create_autocmd("User", {
  group = group, pattern = "NvThemeReload", callback = _G.__zvim_terminal_theme_refresh,
})
_G.__zvim_terminal_theme_refresh()
