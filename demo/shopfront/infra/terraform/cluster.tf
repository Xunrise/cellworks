resource "google_container_cluster" "shop" {
  name               = "${var.project}-cluster"
  location           = var.region
  initial_node_count = 2

  node_config {
    machine_type = "e2-standard-2"
  }
}
