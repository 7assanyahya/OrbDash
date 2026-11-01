// OrbDash case: a small desk console for an Arduino Uno and a 1.28" round
// GC9A01 display. The display sits in a 20 deg tilted front face, the Uno
// lies flat inside with its USB and power jacks at the back, and the
// optional button and LED go through the top.
//
// Parts (pick one with -D part="..." or the Customizer):
//   shell     - top, sides and tilted front. Print upside down (as exported).
//   plate     - base plate with the Uno standoffs and rubber-feet recesses.
//   retainer  - C-ring that presses in behind the display to hold it.
//   assembly  - everything together with dummy Uno/display, for checking.
//
// MEASURE YOUR DISPLAY MODULE and adjust the "Display" values below:
// round GC9A01 boards differ by a millimetre or two.
//
// Hardware: 4x M3x8 screws (plate to shell), 4x M3x6 screws (Uno to
// standoffs), optional 7 mm panel push button, 5 mm LED, 4 rubber feet.
//
// Axes: x across the width (centered), y from the front (0) to the back,
// z up. The Uno's USB end faces the back.

part = "assembly"; // [assembly, shell, plate, retainer]

/* [Display] */
pcb_d    = 39.0;  // round display PCB diameter
pcb_t    = 1.6;   // PCB thickness
glass_d  = 35.6;  // diameter of the glass/panel on the front of the PCB
glass_t  = 2.0;   // thickness of the glass/panel
view_d   = 32.6;  // visible screen area (window size)
header_gap = 26;  // gap in the retainer ring for the pin header and wires

/* [Case] */
tilt     = 20;    // front face lean back from vertical, degrees
H        = 76;    // total height
wall     = 2.4;
plate_t  = 2.4;
corner_r = 5;
disp_z   = 48;    // height of the display center on the front face
fit      = 0.3;   // clearance for press fits

/* [Top panel] */
btn_d    = 7.2;   // hole for a 7 mm panel push button (12 mm button: 12.2)
led_d    = 5.1;   // hole for a 5 mm LED (3 mm LED: 3.1)
engrave  = true;  // "ORBDASH" engraved on top

/* [Hidden] */
$fn = 72;
uno_l = 68.6;  uno_w = 53.3;  uno_t = 1.6;
standoff_h = 5;
uno_y0  = 16;                               // front edge of the Uno PCB
inner_w = uno_w + 6;
W       = inner_w + 2 * wall;
D       = uno_y0 + uno_l + 6.9 + wall;      // USB jack sticks out 6.3 mm
tt      = tan(tilt);
face_t  = wall;                             // front wall, measured normal to it
pcb_z   = plate_t + standoff_h;             // bottom of the Uno PCB
boss_depth = glass_t + pcb_t + 3 + 0.5;     // glass + PCB + retainer
pocket_r = pcb_d / 2 + fit;

// Uno mounting holes (Arduino drawing, USB end at x=0) -> case coordinates
uno_holes = [[13.97, 2.54], [15.24, 50.8], [66.04, 7.62], [66.04, 35.56]];
function uno_to_case(p) = [p[1] - uno_w / 2, uno_y0 + uno_l - p[0]];

// Corner screw posts: front pair just ahead of the Uno, back pair beside the jacks
post_d = 6;
posts = [for (sx = [-1, 1]) for (y = [10, D - wall - 2.5]) [sx * (inner_w / 2 - 2.5), y]];  // merge 0.5 mm into the walls

// ---------------------------------------------------------------------------
// Front face frame: local z = outward normal, local y = up the face
// ---------------------------------------------------------------------------
module on_face() {
    translate([0, disp_z * tt, disp_z]) rotate([90 - tilt, 0, 0]) children();
}

// Rounded body following the tilted front: hull of thin discs at the corners
module body(inset, z0, z1) {
    r = corner_r - inset;
    hull() for (sx = [-1, 1]) {
        for (z = [z0, z1]) {
            translate([sx * (W / 2 - corner_r), z * tt + corner_r + inset / cos(tilt) - inset, z])
                cylinder(r = r, h = 0.01);
            translate([sx * (W / 2 - corner_r), D - corner_r, z]) cylinder(r = r, h = 0.01);
        }
    }
}

module cavity() { body(wall, plate_t - 1, H - wall); }

// ---------------------------------------------------------------------------
// Shell
// ---------------------------------------------------------------------------
module shell() {
    difference() {
        union() {
            // hollow out the body, leaving the corner screw posts standing
            difference() {
                body(0, plate_t, H);
                difference() {
                    cavity();
                    for (p = posts) translate([p[0], p[1], 0]) cylinder(d = post_d, h = H + 1);
                }
            }
            // display boss behind the front face. The hull ramps its upper side
            // back to the face, so it needs no supports when the shell is
            // printed upside down.
            intersection() {
                body(0, plate_t, H);
                on_face() hull() {
                    translate([0, 0, -face_t - boss_depth]) cylinder(r = pocket_r + 1.8, h = boss_depth + 0.01);
                    translate([0, boss_depth + 2, -face_t - 0.01]) cylinder(r = pocket_r + 1.8, h = 0.02);
                }
            }
            // raised ring around the screen
            on_face() translate([0, 0, -0.5]) difference() {
                cylinder(r = view_d / 2 + 4.5, h = 1.7);
                translate([0, 0, -1]) cylinder(r = view_d / 2 + 2.2, h = 3);
            }
        }
        // screen window with a chamfer on the outside
        on_face() {
            translate([0, 0, -face_t - 1]) cylinder(d = view_d + 1, h = face_t + 3);
            translate([0, 0, -face_t / 2]) cylinder(d1 = view_d + 1, d2 = view_d + 1 + face_t * 2, h = face_t / 2 + 1.3);
            // glass pocket, then PCB + retainer pocket
            translate([0, 0, -face_t - glass_t - 0.01]) cylinder(r = glass_d / 2 + fit, h = glass_t + 0.02);
            translate([0, 0, -face_t - boss_depth - 1]) cylinder(r = pocket_r, h = boss_depth - glass_t + 1);
        }
        // pilot holes for M3 screws from below
        for (p = posts) translate([p[0], p[1], plate_t - 1]) cylinder(d = 2.6, h = 13);
        // USB-B and DC jack openings in the back wall (open at the bottom rim)
        jack_hole(37.9, 13.5);  // USB
        jack_hole(7.8, 10.5);   // DC barrel
        // top panel: button, LED, engraving
        translate([14, D - 16, H - wall - 1]) cylinder(d = btn_d, h = wall + 2);
        translate([-14, D - 16, H - wall - 1]) cylinder(d = led_d, h = wall + 2);
        if (engrave)
            translate([0, (H * tt + D) / 2 + 2, H - 0.6])
                linear_extrude(1) text("ORBDASH", size = 7, halign = "center", valign = "center",
                                       font = "Liberation Sans:style=Bold", spacing = 1.15);
    }
}

// Opening for a jack centered at Uno y-coordinate `uy`
module jack_hole(uy, w) {
    translate([uy - uno_w / 2 - w / 2, D - wall - 1, plate_t - 1])
        cube([w, wall + 2, pcb_z + uno_t + 12.5 - plate_t + 1]);
}

// ---------------------------------------------------------------------------
// Base plate
// ---------------------------------------------------------------------------
module plate() {
    difference() {
        union() {
            body(0, 0, plate_t);
            // Uno standoffs
            for (h = uno_holes) let(p = uno_to_case(h))
                translate([p[0], p[1], 0]) cylinder(d = 6, h = plate_t + standoff_h);
        }
        for (h = uno_holes) let(p = uno_to_case(h))
            translate([p[0], p[1], 0.8]) cylinder(d = 2.6, h = 20);
        // countersunk M3 holes into the shell posts
        for (p = posts) translate([p[0], p[1], -1]) {
            cylinder(d = 3.4, h = 10);
            cylinder(d1 = 7, d2 = 3.4, h = 2.8);
        }
        // rubber feet recesses
        for (sx = [-1, 1]) for (y = [12, D - 12])
            translate([sx * (W / 2 - 10), y, -0.01]) cylinder(d = 10.5, h = 1);
    }
}

// ---------------------------------------------------------------------------
// Display retainer: a C-ring pressed in behind the display PCB
// ---------------------------------------------------------------------------
module retainer() {
    difference() {
        cylinder(r = pocket_r - fit / 2, h = 3);
        translate([0, 0, -1]) cylinder(r = pocket_r - 4, h = 5);
        // gap for the pin header and wires (local -y = bottom of the display)
        translate([-header_gap / 2, -pocket_r - 1, -1]) cube([header_gap, pocket_r, 5]);
    }
}

// ---------------------------------------------------------------------------
// Dummies for the assembly view
// ---------------------------------------------------------------------------
module dummy_uno() {
    translate([-uno_w / 2, uno_y0, pcb_z]) {
        color("teal") cube([uno_w, uno_l, uno_t]);
        // USB-B and DC jacks at the back
        color("silver") translate([37.9 - 6, uno_l - 16, uno_t]) cube([12, 22.3, 10.9]);
        color("black") translate([7.8 - 4.5, uno_l - 14, uno_t]) cube([9, 15.8, 11]);
        // headers along both long edges
        color("dimgray") translate([uno_w - 2.5, 4, uno_t]) cube([2.5, 47, 8.5]);
        color("dimgray") translate([0, 18, uno_t]) cube([2.5, 34, 8.5]);
        color("black") translate([20, 8, uno_t]) cube([10, 36, 9]);   // DIP chip
    }
}

module dummy_display() {
    on_face() {
        color("navy") translate([0, 0, -face_t - glass_t - pcb_t]) cylinder(d = pcb_d, h = pcb_t);
        color("black") translate([0, 0, -face_t - glass_t]) cylinder(d = glass_d, h = glass_t);
        // pin header with dupont plugs pointing back
        color("gold") translate([-9, -pcb_d / 2 + 2, -face_t - glass_t - pcb_t - 20]) cube([18, 2.5, 20]);
    }
}

// ---------------------------------------------------------------------------
if (part == "shell") {
    // upside down: the flat top on the bed
    translate([0, 0, H]) rotate([180, 0, 0]) shell();
} else if (part == "plate") {
    plate();
} else if (part == "retainer") {
    retainer();
} else if (part == "showcase") {
    // closed case with the screen showing, for pictures
    color("whitesmoke") plate();
    color("darkslategray") shell();
    on_face() {
        color("black") translate([0, 0, -face_t - glass_t]) cylinder(d = glass_d, h = glass_t);
        color("coral") translate([0, 0, -face_t + 0.05]) difference() {
            cylinder(d = view_d - 6, h = 0.1);
            translate([0, 0, -1]) cylinder(d = view_d - 10, h = 2);
            translate([0, 0, -1]) linear_extrude(3) polygon([[0, 0], [-30, -30], [30, -30]]);
        }
    }
} else if (part == "assembly") {
    color("whitesmoke") plate();
    dummy_uno();
    dummy_display();
    on_face() color("orange") translate([0, 0, -face_t - glass_t - pcb_t - 3]) retainer();
    // cut away the left half of the shell to show the inside
    color("darkslategray", 0.9) difference() {
        shell();
        translate([-W, -10, -1]) cube([W, D + 20, H + 10]);
    }
}
