// OrbDash Pro case: three 1.28" round GC9A01 displays in a row on a tilted
// front, like a car's dash, driven by an ESP32-S3 DevKit at the back.
// About 142 x 86 x 58 mm.
//
// Parts (-D part="..." or the Customizer):
//   shell     - top, sides and tilted front. Print upside down (as exported).
//   plate     - base plate with a fence that locates the DevKit.
//   retainer  - C-ring that holds a display in from behind (print three).
//   assembly  - cut-away with dummies, for checking.
//   showcase  - closed case, for pictures.
//
// The displays' header tabs point up the face. The DevKit lies upside down
// (pins up) on 5 mm foam tape inside the fence, USB ports to the back hole,
// so dupont plugs go straight onto its pins.
// Hardware: 4x M3x8 screws (plate to shell), optional 7 mm push button.

part = "assembly"; // [assembly, shell, plate, retainer, showcase]

include <../lib/orbdash_body.scad>

/* [Pro] */
spacing = 46;     // centre to centre of the displays
H       = 58;
D       = 86;
disp_z  = 28;     // height of the display centres on the front face
btn_d   = 7.2;    // optional push button on top (GPIO0 and GND); 0 = none

/* [ESP32-S3 DevKit] */
kit_l  = 63.5;    // board length (USB end at the back)
kit_w  = 25.6;
foam   = 5;       // foam tape under the upside-down board
usb_hole_w = 26;  // both USB-C ports
usb_hole_h = 11;

/* [Hidden] */
W       = 2 * (spacing + pocket_r + 1.8) + 2 * wall + 2;
inner_w = W - 2 * wall;
post_d  = 6;
posts   = [for (sx = [-1, 1]) for (y = [11, D - wall - 2.5]) [sx * (inner_w / 2 - 2.5), y]];
xs      = [-spacing, 0, spacing];
kit_y0  = D - wall - 0.6 - kit_l;

module cavity() { body(W, D, wall, plate_t - 1, H - wall); }

module shell() {
    difference() {
        union() {
            difference() {
                body(W, D, 0, plate_t, H);
                difference() {
                    cavity();
                    for (p = posts) translate([p[0], p[1], 0]) cylinder(d = post_d, h = H + 1);
                }
            }
            intersection() {
                body(W, D, 0, plate_t, H);
                for (x = xs) on_face(x, disp_z) display_boss();
            }
            for (x = xs) on_face(x, disp_z) display_ring();
        }
        for (x = xs) on_face(x, disp_z) display_cut();
        for (p = posts) translate([p[0], p[1], plate_t - 1]) cylinder(d = 2.6, h = 13);
        // both USB-C ports of the DevKit, in the back wall
        translate([-usb_hole_w / 2, D - wall - 1, plate_t - 0.01]) cube([usb_hole_w, wall + 2, usb_hole_h]);
        if (btn_d > 0) translate([spacing, D - 14, H - wall - 1]) cylinder(d = btn_d, h = wall + 2);
        // engraving on top, like the original OrbDash
        translate([-spacing / 2, D / 2 + H * tt / 2, H - 0.6])
            linear_extrude(1) text("ORBDASH PRO", size = 6, halign = "center", valign = "center",
                                   font = "Liberation Sans:style=Bold", spacing = 1.15);
    }
}

module plate() {
    difference() {
        union() {
            body(W, D, 0, 0, plate_t);
            // fence that locates the DevKit's foam pad
            translate([0, kit_y0 + kit_l / 2, plate_t]) difference() {
                translate([-kit_w / 2 - fit - 1.6, -kit_l / 2 - fit - 1.6, 0]) cube([kit_w + 2 * fit + 3.2, kit_l + 2 * fit + 1.6, 3]);
                translate([-kit_w / 2 - fit, -kit_l / 2 - fit, -1]) cube([kit_w + 2 * fit, kit_l + 2 * fit + 2, 5]);
            }
        }
        for (p = posts) plate_screw(p);
        for (sx = [-1, 1]) for (y = [12, D - 12])
            translate([sx * (W / 2 - 12), y, -0.01]) cylinder(d = 10.5, h = 1);   // rubber feet
    }
}

module dummy_kit() {
    translate([-kit_w / 2, kit_y0, plate_t + foam]) {
        color("white", 0.6) translate([0, 0, -foam]) cube([kit_w, kit_l, foam]);
        color("black") cube([kit_w, kit_l, 1.6]);
        color("silver") for (x = [4, kit_w - 13]) translate([x, kit_l - 7.3, -3.2]) cube([9, 7.5, 3.2]);
        color("gold") for (x = [1, kit_w - 3.5]) translate([x, 4, 1.6]) cube([2.5, kit_l - 8, 8.5]);  // pins up
    }
}

if (part == "shell") {
    translate([0, 0, H]) rotate([180, 0, 0]) shell();
} else if (part == "plate") {
    plate();
} else if (part == "retainer") {
    retainer();
} else if (part == "showcase") {
    color("whitesmoke") plate();
    color("#2b2d38") shell();
    for (x = xs) on_face(x, disp_z) color("black") translate([0, 0, -face_t - glass_t]) cylinder(d = glass_d, h = glass_t);
} else {
    color("whitesmoke") plate();
    dummy_kit();
    for (x = xs) on_face(x, disp_z) {
        dummy_display();
        color("orange") translate([0, 0, -face_t - glass_t - pcb_t - 3]) retainer();
    }
    // cut through the middle display to show the inside
    color("#2b2d38", 0.9) difference() {
        shell();
        translate([-W, -10, -1]) cube([W, D + 20, H + 10]);
    }
}
