local desired = ...
local state = _G.__zvim_appearance
-- Base46 installs highlights directly, without a :colorscheme that Neovim can
-- reload after changing 'background'. A background change resets those colours
-- to defaults; retain the selected palette instead of inventing a light variant.
local nvconfig = package.loaded.nvconfig
if nvconfig and nvconfig.base46 and vim.g.base46_cache and not vim.g.colors_name then
  if state then
    -- Repair highlights if a previous version already changed the background.
    require("base46").load_all_highlights()
  end
  _G.__zvim_appearance = nil
  return
end
if desired == vim.NIL then
  if state and vim.o.background == state.applied then
    vim.o.background = state.previous
  end
  _G.__zvim_appearance = nil
else
  state = state or { previous = vim.o.background }
  state.applied = desired
  _G.__zvim_appearance = state
  if vim.o.background ~= desired then
    vim.o.background = desired
  end
end
