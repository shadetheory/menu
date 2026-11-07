# Outside click

An outside click dismisses the menu and is not delivered to the window underneath. The seat test on headless Sway measured that.

The menu is the panel-sized layer surface `omarchy-menu-rust`. A visible dim layer, `omarchy-menu-rust-dim`, covers only the focused output while the menu is open. A press on that dim closes the menu and is consumed by this process. Sway 1.9 calls `wlr_xdg_popup_unconstrain_from_box` when a layer popup is created, before the client's first commit, and wlroots then logs `A configure is scheduled for an uninitialized xdg_surface`. This client does not create an `xdg_popup`, so that configure is not scheduled. The dim is destroyed when the process exits.

The earlier popup grab is not used. Sway rejected a grab sent after the popup was configured, and a grab sent before the first commit was the order that produced the uninitialized-configure log. Keyboard input stays on the menu layer. Escape goes back one level, then closes. A press outside the pill is a press on the dim.
