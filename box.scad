// Open-top plywood box: bottom + four sides
// All dimensions are in millimeters.

box_length = 175;
box_width  = 80;
box_height = 143;
thickness  = 6;
inner_length = box_length - 2 * thickness;
inner_width = box_width - 2 * thickness;
tunnel_height = 45; // 68 mm * 45 mm = 30.6 cm^2
second_floor_z = thickness + tunnel_height;
floor_passage_length = 45; // 68 mm * 45 mm = 30.6 cm^2
second_floor_length = inner_length - floor_passage_length;

pcb_length = 85;
pcb_width = 35;
pcb_thickness = 2;
pcb_standoff = 3;
pcb_back_offset = 5;
pcb_x = thickness
    + (second_floor_length - pcb_length) / 2
    + pcb_back_offset;
pcb_y = thickness + (inner_width - pcb_width) / 2;

cylinder_diameter = 20;
cylinder_height = 12;
cylinder_end_offset = 15;
propeller_diameter = 60;
propeller_height = 8;
propeller_top_clearance = 10;
third_floor_z = second_floor_z
    + thickness
    + pcb_standoff
    + pcb_thickness
    + cylinder_height
    + propeller_height
    + propeller_top_clearance;

// Set above zero for an exploded view.
explode = 0;

module bottom_panel() {
    translate([0, 0, -explode])
        cube([box_length, box_width, thickness]);
}

module top_panel() {
    translate([0, 0, box_height + explode])
        cube([box_length, box_width, thickness]);
}

// Long side at y = 0.
module left_panel() {
    translate([0, -explode, thickness])
        cube([
            box_length,
            thickness,
            box_height - thickness
        ]);
}

// Long side at y = box_width.
module right_panel() {
    translate([
        0,
        box_width - thickness + explode,
        thickness
    ])
        cube([
            box_length,
            thickness,
            box_height - thickness
        ]);
}

// Short end at x = 0.
module front_panel() {
    translate([
        -explode,
        thickness,
        thickness
    ])
        difference() {
            cube([
                thickness,
                inner_width,
                box_height - thickness
            ]);

            // Outlet below the second floor: 68 mm * 45 mm = 30.6 cm^2.
            translate([-0.1, -0.1, -0.1])
                cube([
                    thickness + 0.2,
                    inner_width + 0.2,
                    tunnel_height + 0.1
                ]);
        }
}

// Short end at x = box_length.
module back_panel() {
    translate([
        box_length - thickness + explode,
        thickness,
        thickness
    ])
        difference() {
            cube([
                thickness,
                inner_width,
                box_height - thickness
            ]);

            // Outlet above the third floor: 68 mm * 45 mm = 30.6 cm^2.
            translate([
                -0.1,
                -0.1,
                third_floor_z
            ])
                cube([
                    thickness + 0.2,
                    inner_width + 0.2,
                    tunnel_height
                ]);
        }
}

module second_floor_panel() {
    translate([
        thickness,
        thickness,
        second_floor_z + explode
    ])
        cube([
            second_floor_length,
            inner_width,
            thickness
        ]);
}

module third_floor_panel() {
    translate([
        thickness + floor_passage_length,
        thickness,
        third_floor_z + explode
    ])
        cube([
            second_floor_length,
            inner_width,
            thickness
        ]);
}

module mock_pcb() {
    translate([
        pcb_x,
        pcb_y,
        second_floor_z + thickness + pcb_standoff + explode
    ])
        cube([
            pcb_length,
            pcb_width,
            pcb_thickness
        ]);
}

module mock_cylinder() {
    translate([
        pcb_x + cylinder_end_offset,
        pcb_y + pcb_width / 2,
        second_floor_z + thickness + pcb_standoff + pcb_thickness + explode
    ])
        cylinder(
            h = cylinder_height,
            d = cylinder_diameter,
            $fn = 64
        );
}

module mock_propeller() {
    translate([
        pcb_x + cylinder_end_offset,
        pcb_y + pcb_width / 2,
        second_floor_z
            + thickness
            + pcb_standoff
            + pcb_thickness
            + cylinder_height
            + explode
    ])
        cylinder(
            h = propeller_height,
            d = propeller_diameter,
            $fn = 96
        );
}

color("BurlyWood") {
    bottom_panel();
    top_panel();
    //left_panel();
    right_panel();
    front_panel();
    back_panel();
    second_floor_panel();
    third_floor_panel();
}

color("ForestGreen")
    mock_pcb();

color("Black")
    mock_cylinder();

// Alpha 0.8 means 20% transparent.
color([0.7, 0.7, 0.7, 0.8])
    mock_propeller();
