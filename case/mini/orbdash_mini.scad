// OrbDash Mini case: one 1.28" round GC9A01 display in a small tilted pod,
// driven by an ESP32-C3 SuperMini lying in a pocket on the base plate with
// its USB-C facing the back. About 50 x 52 x 56 mm.
//
// Parts (-D part="..." or the Customizer):
//   shell     - top, sides and tilted front. Print upside down (as exported).
//   plate     - base plate with the board pocket.
//   retainer  - C-ring that holds the display in from behind.
//   assembly  - cut-away with a dummy board and display, for checking.
//   showcase  - closed case, for pictures.
//
// The display's header tab points up the face. Solder short wires to the
// SuperMini (dupont plugs fit too, but take more room).
// Hardware: 4x M3x8 screws (plate to shell), optional 7 mm push button.

part = "assembly"; // [assembly, shell, plate, retainer, showcase]

include <../lib/orbdash_body.scad>

/* [Mini] */
W      = 50;
D      = 52;
H      = 56;
disp_z = 27;      // height of the display centre on the front face
btn_d  = 7.2;     // optional push button on top (wired to GPIO9 and GND); 0 = none

/* [ESP32-C3 SuperMini] */
mcu_l  = 22.6;    // board length (USB end at the back)
mcu_w  = 18.0;
mcu_t  = 1.0;
usb_w  = 9.2;     // USB-C receptacle
usb_h  = 3.3;

/* [Hidden] */
inner_w = W - 2 * wall;
post_d  = 6;
posts   = [for (sx = [-1, 1]) for (y = [11, D - wall - 2.5]) [sx * (inner_w / 2 - 2.5), y]];
mcu_y0  = D - wall - 0.4 - mcu_l;   // the USB end sits against the back wall

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
                on_face(0, disp_z) display_boss();
            }
            on_face(0, disp_z) display_ring();
        }
        on_face(0, disp_z) display_cut();
        for (p = posts) translate([p[0], p[1], plate_t - 1]) cylinder(d = 2.6, h = 13);
        // USB-C opening in the back wall, roomy enough for the plug's overmould
        translate([-6.5, D - wall - 1, plate_t - 0.01]) cube([13, wall + 2, mcu_t + usb_h + 5]);
        if (btn_d > 0) translate([0, D - 12, H - wall - 1]) cylinder(d = btn_d, h = wall + 2);
    }
}

module plate() {
    difference() {
        union() {
            body(W, D, 0, 0, plate_t);
            // low fence round the SuperMini
            translate([0, mcu_y0 + mcu_l / 2, plate_t]) difference() {
                translate([-mcu_w / 2 - fit - 1.6, -mcu_l / 2 - fit - 1.6, 0]) cube([mcu_w + 2 * fit + 3.2, mcu_l + 2 * fit + 1.6, 2]);
                translate([-mcu_w / 2 - fit, -mcu_l / 2 - fit, -1]) cube([mcu_w + 2 * fit, mcu_l + 2 * fit + 2, 4]);
            }
        }
        for (p = posts) plate_screw(p);
    }
}

module dummy_mcu() {
    translate([-mcu_w / 2, mcu_y0, plate_t]) {
        color("black") cube([mcu_w, mcu_l, mcu_t]);
        color("silver") translate([mcu_w / 2 - usb_w / 2, mcu_l - 7.3, mcu_t]) cube([usb_w, 7.5, usb_h]);
        color("dimgray") translate([3, 3, mcu_t]) cube([10, 10, 1.2]);   // ESP32-C3 chip
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
    on_face(0, disp_z) color("black") translate([0, 0, -face_t - glass_t]) cylinder(d = glass_d, h = glass_t);
} else {
    color("whitesmoke") plate();
    dummy_mcu();
    on_face(0, disp_z) {
        dummy_display();
        color("orange") translate([0, 0, -face_t - glass_t - pcb_t - 3]) retainer();
    }
    color("#2b2d38", 0.9) difference() {
        shell();
        translate([-W, -10, -1]) cube([W, D + 20, H + 10]);
    }
}
