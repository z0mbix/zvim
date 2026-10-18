-- Project identity belongs to Neovim; the frontend only groups native terminals.
local channel = ...
local explicit_root
local pending = false
local last_root
local M = {}

local function root(path)
  assert(type(path) == 'string' and path ~= '', 'Zvim project root must be a directory')
  path = vim.fn.fnamemodify(path, ':p'):gsub('/+$', '')
  if path == '' then path = '/' end
  path = vim.uv.fs_realpath(path) or path
  assert(vim.fn.isdirectory(path) == 1, 'Zvim project root does not exist: ' .. path)
  return path
end

local function publish()
  local path = explicit_root or root(vim.fn.getcwd(-1, -1))
  if path ~= last_root then
    last_root = path
    vim.rpcnotify(channel, 'zvim_project', path)
  end
end

local function schedule()
  if pending then return end
  pending = true
  vim.schedule(function()
    pending = false
    publish()
  end)
end

function M.set_project(path)
  explicit_root = path and root(path) or nil
  publish()
end

function M.project_root()
  return explicit_root or root(vim.fn.getcwd(-1, -1))
end

-- Terminal requests flush project identity before reporting their directory.
-- This also orders calls made inside one project plugin callback correctly.
_G.__zvim_project_refresh = publish
package.loaded['zvim'] = M
vim.api.nvim_create_user_command('ZvimProject', function(opts)
  M.set_project(opts.args ~= '' and opts.args or vim.fn.getcwd(-1, -1))
end, { nargs = '?', complete = 'dir', desc = 'Select the project for native terminal layouts' })
local group = vim.api.nvim_create_augroup('ZvimProject', { clear = true })
vim.api.nvim_create_autocmd('DirChanged', {
  group = group,
  pattern = 'global',
  callback = function()
    if not explicit_root then schedule() end
  end,
})
vim.api.nvim_create_autocmd({ 'VimEnter', 'SessionLoadPost' }, { group = group, callback = schedule })
if vim.v.vim_did_enter == 1 then schedule() end
