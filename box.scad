// Layered plywood airflow box built from rectangular panels.
// All dimensions are in millimeters.

box_length = 175;
box_width  = 80;
thickness  = 6;

inner_length = box_length - 2 * thickness;
inner_width = box_width - 2 * thickness;

minimum_passage_area = 4000; // 40 cm^2
passage_size = ceil(minimum_passage_area / inner_width);
passage_area = inner_width * passage_size; // 68 mm * 59 mm = 40.12 cm^2
propeller_window_length = 68;
propeller_window_area = inner_width * propeller_window_length; // 46.24 cm^2

second_floor_z = thickness + passage_size;
second_floor_length = inner_length - passage_size;

middle_passage_height = passage_size;
third_floor_z = second_floor_z + thickness + middle_passage_height;
third_floor_length = inner_length - propeller_window_length;

upper_passage_height = passage_size;
fourth_floor_z = third_floor_z + thickness + upper_passage_height;
fourth_floor_length = inner_length - passage_size;

top_passage_height = passage_size;
box_height = fourth_floor_z + thickness + top_passage_height;

pcb_length = 85;
pcb_width = 35;
pcb_thickness = 2;
pcb_standoff = 3;
cylinder_diameter = 20;
cylinder_height = 12;
cylinder_end_offset = 15;
propeller_diameter = 60;
propeller_height = 8;

// Center the propeller in the 68 mm square opening.
pcb_x = thickness
    + propeller_window_length / 2
    - cylinder_end_offset;
pcb_y = thickness + (inner_width - pcb_width) / 2;

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
        second_floor_z
    ])
        cube([
            thickness,
            inner_width,
            fourth_floor_z + thickness - second_floor_z
        ]);
}

// Short end at x = box_length.
module back_panel() {
    translate([
        box_length - thickness + explode,
        thickness,
        thickness
    ])
        cube([
            thickness,
            inner_width,
            box_height - thickness
        ]);
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
        thickness + propeller_window_length,
        thickness,
        third_floor_z + explode
    ])
        cube([
            third_floor_length,
            inner_width,
            thickness
        ]);
}

module fourth_floor_panel() {
    translate([
        thickness,
        thickness,
        fourth_floor_z + explode
    ])
        cube([
            fourth_floor_length,
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
    fourth_floor_panel();
}

color("ForestGreen")
    mock_pcb();

color("Black")
    mock_cylinder();

// Alpha 0.8 means 20% transparent.
color([0.7, 0.7, 0.7, 0.8])
    mock_propeller();
