local channel = ...
vim.g.clipboard = {
  name = 'Zvim',
  copy = {
    ['+'] = function(lines, regtype) vim.rpcnotify(channel, 'zvim_clipboard_copy', lines, regtype) end,
    ['*'] = function(lines, regtype) vim.rpcnotify(channel, 'zvim_clipboard_copy', lines, regtype) end,
  },
  paste = {
    ['+'] = function() return vim.rpcrequest(channel, 'zvim_clipboard_paste') end,
    ['*'] = function() return vim.rpcrequest(channel, 'zvim_clipboard_paste') end,
  },
  cache_enabled = 0,
}
