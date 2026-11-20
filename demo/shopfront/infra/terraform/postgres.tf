resource "google_sql_database_instance" "orders" {
  name             = "${var.project}-orders"
  region           = var.region
  database_version = "POSTGRES_15"

  settings {
    tier = var.db_tier
  }
}

resource "google_sql_database" "orders" {
  name     = "orders"
  instance = google_sql_database_instance.orders.name
}
