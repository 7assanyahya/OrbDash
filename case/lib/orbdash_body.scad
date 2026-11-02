// Shared parts of the OrbDash Mini and Pro cases: the tilted console body,
// the display pocket (round board plus header tab), the retainer ring and a
// dummy display. Include it, then set the case's own sizes (variables set in
// the including file override the defaults here).
//
// Axes: x across the width (centered), y from the front (0) to the back,
// z up. The front face leans back by `tilt`.

/* [Display] */
pcb_d     = 39.0;  // round display PCB diameter
pcb_t     = 1.6;   // PCB thickness
glass_d   = 35.6;  // glass/panel diameter on the front of the PCB
glass_t   = 2.0;   // glass/panel thickness
view_d    = 32.6;  // visible screen area (window size)
tab_w     = 26.0;  // width of the header tab, measured across it
tab_reach = 27.0;  // from the centre of the round PCB to the far edge of the tab
joint_h   = 2.0;   // how far the header's solder joints stick out on the screen side
tab_up    = true;  // tab points up the face (the firmware turns the picture to match)

/* [Case] */
tilt     = 20;
wall     = 2.4;
plate_t  = 2.4;
corner_r = 5;
fit      = 0.3;

/* [Hidden] */
$fn = 72;
tt = tan(tilt);
face_t = wall;
boss_depth = glass_t + pcb_t + 3 + 0.5;   // glass + PCB + retainer
pocket_r = pcb_d / 2 + fit;
tab_dir = tab_up ? 1 : -1;

// Front face frame at height z and x offset: local z = outward normal,
// local y = up the face
module on_face(x, z) {
    translate([x, z * tt, z]) rotate([90 - tilt, 0, 0]) children();
}

// Rounded body following the tilted front: hull of thin discs at the corners
module body(W, D, inset, z0, z1) {
    r = corner_r - inset;
    hull() for (sx = [-1, 1]) for (z = [z0, z1]) {
        translate([sx * (W / 2 - corner_r), z * tt + corner_r + inset / cos(tilt) - inset, z]) cylinder(r = r, h = 0.01);
        translate([sx * (W / 2 - corner_r), D - corner_r, z]) cylinder(r = r, h = 0.01);
    }
}

// Material behind the face that the display pocket is cut from. The hull
// ramps its top back to the face so it prints without supports upside down.
module display_boss() {
    hull() {
        translate([0, 0, -face_t - boss_depth]) cylinder(r = pocket_r + 1.8, h = boss_depth + 0.01);
        translate([0, boss_depth + 2, -face_t - 0.01]) cylinder(r = pocket_r + 1.8, h = 0.02);
    }
    // Material round the tab slot so the board's tab is held. With the tab
    // up it runs on up the face into the top wall (the shell clips it), so
    // when the shell prints upside down it has no overhanging end.
    if (tab_up)
        translate([-tab_w / 2 - fit - 1.8, 0, -face_t - boss_depth]) cube([tab_w + 2 * fit + 3.6, 90, boss_depth + 0.01]);
}

// Raised ring round the screen, on the outside of the face
module display_ring() {
    translate([0, 0, -0.5]) difference() {
        cylinder(r = view_d / 2 + 4.5, h = 1.7);
        translate([0, 0, -1]) cylinder(r = view_d / 2 + 2.2, h = 3);
    }
}

// Window, glass pocket, PCB pocket and the slot for the header tab
module display_cut() {
    translate([0, 0, -face_t - 1]) cylinder(d = view_d + 1, h = face_t + 3);
    translate([0, 0, -face_t / 2]) cylinder(d1 = view_d + 1, d2 = view_d + 1 + face_t * 2, h = face_t / 2 + 1.3);
    translate([0, 0, -face_t - glass_t - 0.01]) cylinder(r = glass_d / 2 + fit, h = glass_t + 0.02);
    translate([0, 0, -face_t - boss_depth - 1]) cylinder(r = pocket_r, h = boss_depth - glass_t + 1);
    // tab slot, reaching forward of the PCB for the solder joints and the
    // folded display ribbon, always leaving >= 1.2 mm of front wall
    tab_top = min(-face_t - glass_t + joint_h, -1.2);
    translate([-tab_w / 2 - fit, tab_up ? 0 : -tab_reach - fit, -face_t - boss_depth - 1.01])
        cube([tab_w + 2 * fit, tab_reach + fit, tab_top + face_t + boss_depth + 1.01]);
}

// C-ring pressed in behind the display PCB; its gap leaves room for the tab
module retainer() {
    rotate([0, 0, tab_up ? 180 : 0]) difference() {
        cylinder(r = pocket_r - fit / 2, h = 3);
        translate([0, 0, -1]) cylinder(r = pocket_r - 4, h = 5);
        translate([-tab_w / 2, -pocket_r - 1, -1]) cube([tab_w, pocket_r, 5]);
    }
}

module dummy_display() {
    color("navy") translate([0, 0, -face_t - glass_t - pcb_t]) {
        cylinder(d = pcb_d, h = pcb_t);
        translate([-tab_w / 2, tab_up ? 0 : -tab_reach, 0]) cube([tab_w, tab_reach, pcb_t]);
    }
    color("black") translate([0, 0, -face_t - glass_t]) cylinder(d = glass_d, h = glass_t);
    // header and dupont plugs pointing back into the case
    color("gold") translate([-9, tab_dir * (tab_reach - 2) - (tab_up ? 2.5 : 0), -face_t - glass_t - pcb_t - 20])
        cube([18, 2.5, 20]);
}

// Countersunk M3 screw hole up into a post, from the bottom of the plate
module plate_screw(p) {
    translate([p[0], p[1], -1]) {
        cylinder(d = 3.4, h = 10);
        cylinder(d1 = 7, d2 = 3.4, h = 2.8);
    }
}
