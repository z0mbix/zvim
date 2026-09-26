-- Isolates the plugin's synchronous resize callback; does not measure GUI frames.
-- Run from the repository root with MINIMAP_BENCH_MODE=off|on|no_git|sh.
local mode = vim.env.MINIMAP_BENCH_MODE or 'on'
assert(vim.tbl_contains({'off', 'on', 'no_git', 'sh'}, mode), 'Invalid benchmark mode')
vim.fn.mkdir('.cache/minimap', 'p')
local root = vim.fn.getcwd()
if mode == 'sh' then vim.o.shell = '/bin/sh'; vim.o.shellcmdflag = '-c' end
vim.opt.rtp:append(vim.fn.expand('~/.local/share/nvim/lazy/minimap.vim'))
vim.g.minimap_auto_start = 0
vim.g.minimap_width = 16
vim.g.minimap_highlight_range = true
vim.g.minimap_git_colors = mode == 'no_git' and 0 or 1
vim.g.minimap_enable_highlight_colorgroup = 1
vim.cmd('runtime plugin/minimap.vim')
vim.cmd.edit(root .. '/src/ui.rs')
if mode ~= 'off' then vim.cmd.Minimap() end
vim.wait(300)
vim.cmd('profile start ' .. root .. '/.cache/minimap/' .. mode .. '.profile')
vim.cmd('profile func *')
local times = {}
for i=1,40 do
  local start = vim.uv.hrtime()
  vim.o.columns = 140 + i % 20
  vim.o.lines = 40 + i % 15
  vim.cmd('doautocmd VimResized')
  times[#times+1] = (vim.uv.hrtime()-start)/1e6
end
vim.cmd('profile stop')
vim.fn.writefile({vim.json.encode({mode=mode, milliseconds=times, highlight_updates=vim.g.minimap_run_update_highlight_count})}, root .. '/.cache/minimap/' .. mode .. '.json')
vim.cmd('qa!')
