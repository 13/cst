-- catppucin theme

local theme_assets                              = require("beautiful.theme_assets")
local xresources                                = require("beautiful.xresources")
local dpi                                       = xresources.apply_dpi

local gfs                                       = require("gears.filesystem")

local theme_name                                = "catppuccin"
local theme_layout_size                         = "27x27" -- 26x26

local themes_path                               = gfs.get_configuration_dir() .. "themes/" .. theme_name .. "/"
local layout_path                               = themes_path .. "layouts/" .. theme_layout_size .. "/"
local titlebar_path                             = themes_path .. "titlebar/"

local theme                                     = {}

--theme.font                                      = "Ubuntu Nerd Font 11"
--theme.font                                      = "Noto Sans 11"
--theme.font                                      = "UbuntuMono Nerd Font 12"
--theme.font                                      = "JetBrainsMono NF 11" -- NF/NFM/NFP
--theme.font                                      = "Terminus 9"
theme.font                                      = "Liga SFMono Nerd Font 11"
--theme.font                                      = "SFPro Nerd Font Text 11"
--theme.font                                      = "SFPro Nerd Font Display 11"
--theme.font                                      = "NotoMono NF 11"
--theme.font                                      = "DroidSansM Nerd Font 11"

theme.font_alt                                  = "SFPro Nerd Font Text 11"
theme.font_alt2                                 = "SFPro Nerd Font Text 9"
theme.font_alt3                                 = "Ubuntu Nerd Font 28"

theme.rofi                                      = gfs.get_xdg_config_home() .. '/rofi/catppuccin-mocha.rasi'

theme.wallpaper                                 = themes_path .. "wallpapers/wallpaper.jpg"

theme.icon_theme                                = "WhiteSur-dark"
theme.tasklist_disable_icon                     = true

theme.useless_gap                               = dpi(0)
theme.border_width                              = dpi(1)

theme.colors                                    = {}

theme.colors.base00                             = "#1e1e2e"           -- base
theme.colors.base01                             = "#181825"           -- mantle
theme.colors.base02                             = "#313244"           -- surface0
theme.colors.base03                             = "#45475a"           -- surface1
theme.colors.base04                             = "#585b70"           -- surface2
theme.colors.base05                             = "#cdd6f4"           -- text
theme.colors.base06                             = "#f5e0dc"           -- rosewater
theme.colors.base07                             = "#b4befe"           -- lavender
theme.colors.base08                             = "#f38ba8"           -- red
theme.colors.base09                             = "#fab387"           -- peach
theme.colors.base0A                             = "#f9e2af"           -- yellow
theme.colors.base0B                             = "#a6e3a1"           -- green
theme.colors.base0C                             = "#94e2d5"           -- teal
theme.colors.base0D                             = "#89b4fa"           -- blue
theme.colors.base0E                             = "#cba6f7"           -- mauve
theme.colors.base0F                             = "#f2cdcd"           -- flamingo

theme.colors.black                              = theme.colors.base03 -- surface1
theme.colors.white                              = "#eff1f5"           -- text
theme.colors.darkred                            = "#d20f39"           -- red
--theme.colors.base06 = "f5e0dc" -- rosewater
theme.colors.violet                             = theme.colors.base07 -- lavender
theme.colors.red                                = theme.colors.base08 -- red
theme.colors.peach                              = theme.colors.base09 -- peach
theme.colors.orange                             = theme.colors.peach  -- rc.lua widgets use colors.orange
theme.colors.yellow                             = theme.colors.base0A -- yellow
theme.colors.green                              = theme.colors.base0B -- green
theme.colors.cyan                               = theme.colors.base0C -- teal
theme.colors.blue                               = theme.colors.base0D -- blue
theme.colors.magenta                            = theme.colors.base0E -- mauve
--themes.colors.base0F = "f2cdcd"  -- flamingo

theme.fg_normal                                 = theme.colors.base05
theme.fg_focus                                  = theme.colors.base05 -- base05
theme.fg_urgent                                 = theme.colors.base05 -- base01

theme.bg_normal                                 = theme.colors.base00
theme.bg_focus                                  = theme.colors.base04  -- base04
theme.bg_urgent                                 = theme.colors.darkred -- red
theme.bg_systray                                = theme.bg_normal

theme.border_normal                             = theme.bg_normal
theme.border_focus                              = theme.bg_focus
theme.border_marked                             = theme.bg_urgent

theme.titlebar_bg_focus                         = theme.bg_focus
theme.titlebar_bg_normal                        = theme.bg_normal

theme.mouse_finder_color                        = theme.colors.green

theme.hud_slider_fg                             = theme.fg_normal
theme.hud_slider_bg                             = theme.bg_focus
theme.hud_popup_bg                              = theme.bg_normal
theme.hud_popup_border_color                    = theme.border_focus
theme.hud_popup_border_width                    = dpi(1)

-- Notifications (naughty)
theme.notification_font                         = theme.font
theme.notification_bg                           = theme.bg_normal
theme.notification_fg                           = theme.fg_normal
theme.notification_border_width                 = dpi(1)
theme.notification_border_color                 = theme.border_focus
theme.notification_margin                       = dpi(12)
theme.notification_max_width                    = dpi(480)
theme.notification_icon_size                    = dpi(48)

-- There are other variable sets
-- overriding the default one when
-- defined, the sets are:
-- taglist_[bg|fg]_[focus|urgent|occupied|empty|volatile]
-- tasklist_[bg|fg]_[focus|urgent]
-- titlebar_[bg|fg]_[normal|focus]
-- tooltip_[font|opacity|fg_color|bg_color|border_width|border_color]
-- mouse_finder_[color|timeout|animate_timeout|radius|factor]
-- prompt_[fg|bg|fg_cursor|bg_cursor|font]
-- hotkeys_[bg|fg|border_width|border_color|shape|opacity|modifiers_fg|label_bg|label_fg|group_margin|font|description_font]
-- Example:
--theme.taglist_bg_focus = "#ff0000"

-- {{{ Icons
-- {{{ Taglist
--theme.taglist_squares_sel                       = themes_path .. "taglist/squarefw.png"
--theme.taglist_squares_unsel                     = themes_path .. "taglist/square.png"
-- theme.taglist_squares_resize                    = "false"
-- }}}

-- Generate taglist squares:
--local taglist_square_size                       = dpi(6)
--theme.taglist_squares_sel                       = theme_assets.taglist_squares_sel(
--    taglist_square_size, theme.fg_normal
--)
--theme.taglist_squares_unsel                     = theme_assets.taglist_squares_unsel(
-- taglist_square_size, theme.fg_normal
--)

-- Variables set for theming notifications:
-- notification_font
-- notification_[bg|fg]
-- notification_[width|height|margin]
-- notification_[border_color|border_width|shape|opacity]

-- Variables set for theming the menu:
-- menu_[bg|fg]_[normal|focus]
-- menu_[border_color|border_width]
theme.menu_accent                                = "#89b4fa"
-- Same surface, text and highlight as the bar (bg_normal, fg_normal and
-- the bg_focus the bar uses for the focused tag/task), with a quiet
-- border: the menu reads as part of the bar. Glyphs stay in menu_accent.
theme.menu_bg_normal                             = theme.bg_normal
theme.menu_bg_focus                              = theme.bg_focus
theme.menu_fg_normal                             = theme.fg_normal
theme.menu_fg_focus                              = theme.fg_focus
theme.menu_border_color                          = theme.colors.base03
theme.menu_border_width                          = dpi(1)
theme.menu_font                                  = theme.font
theme.menu_height                                = dpi(30)
theme.menu_width                                 = dpi(200)
theme.menu_radius                                = dpi(8)

-- Bar hover cards (components/infocard): a raised surface in the menu's
-- accent, with a quiet border
theme.infocard_bg                               = theme.colors.base02
theme.infocard_accent                           = theme.menu_accent
theme.infocard_border_color                     = theme.colors.base03
theme.infocard_border_width                     = dpi(1)
theme.infocard_radius                           = dpi(12)

-- You can add as many variables as
-- you wish and access them by using
-- beautiful.variable in your rc.lua
--theme.bg_widget = "#cc0000"

-- Define the image to load
theme.titlebar_close_button_normal              = titlebar_path .. "close_normal.png"
theme.titlebar_close_button_focus               = titlebar_path .. "close_focus.png"

theme.titlebar_minimize_button_normal           = titlebar_path .. "minimize_normal.png"
theme.titlebar_minimize_button_focus            = titlebar_path .. "minimize_focus.png"

theme.titlebar_ontop_button_normal_inactive     = titlebar_path .. "ontop_normal_inactive.png"
theme.titlebar_ontop_button_focus_inactive      = titlebar_path .. "ontop_focus_inactive.png"
theme.titlebar_ontop_button_normal_active       = titlebar_path .. "ontop_normal_active.png"
theme.titlebar_ontop_button_focus_active        = titlebar_path .. "ontop_focus_active.png"

theme.titlebar_sticky_button_normal_inactive    = titlebar_path .. "sticky_normal_inactive.png"
theme.titlebar_sticky_button_focus_inactive     = titlebar_path .. "sticky_focus_inactive.png"
theme.titlebar_sticky_button_normal_active      = titlebar_path .. "sticky_normal_active.png"
theme.titlebar_sticky_button_focus_active       = titlebar_path .. "sticky_focus_active.png"

theme.titlebar_floating_button_normal_inactive  = titlebar_path .. "floating_normal_inactive.png"
theme.titlebar_floating_button_focus_inactive   = titlebar_path .. "floating_focus_inactive.png"
theme.titlebar_floating_button_normal_active    = titlebar_path .. "floating_normal_active.png"
theme.titlebar_floating_button_focus_active     = titlebar_path .. "floating_focus_active.png"

theme.titlebar_maximized_button_normal_inactive = titlebar_path .. "maximized_normal_inactive.png"
theme.titlebar_maximized_button_focus_inactive  = titlebar_path .. "maximized_focus_inactive.png"
theme.titlebar_maximized_button_normal_active   = titlebar_path .. "maximized_normal_active.png"
theme.titlebar_maximized_button_focus_active    = titlebar_path .. "maximized_focus_active.png"

--theme.wallpaper = themes_path.."background.png"
--gears.wallpaper.set("#000000")

-- You can use your own layout icons like this:
theme.layout_fairh                              = layout_path .. "fairh.png"
theme.layout_fairv                              = layout_path .. "fairv.png"
theme.layout_floating                           = layout_path .. "floating.png"
theme.layout_max                                = layout_path .. "max.png"
theme.layout_fullscreen                         = layout_path .. "fullscreen.png"
theme.layout_tilebottom                         = layout_path .. "tilebottom.png"
theme.layout_tileleft                           = layout_path .. "tileleft.png"
theme.layout_tile                               = layout_path .. "tile.png"
theme.layout_tiletop                            = layout_path .. "tiletop.png"
theme.layout_dwindle                            = layout_path .. "dwindle.png"

-- Generate Awesome icon:
--theme.awesome_icon                              = theme_assets.awesome_icon(
--    theme.menu_height, theme.bg_focus, theme.fg_focus
--)
theme.awesome_icon                              = themes_path .. "icons/menu3.png"

return theme

-- vim: filetype=lua:expandtab:shiftwidth=4:tabstop=8:softtabstop=4:textwidth=80
