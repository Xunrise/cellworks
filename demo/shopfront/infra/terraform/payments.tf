resource "google_secret_manager_secret" "payments_api_key" {
  secret_id = "${var.project}-payments-api-key"

  replication {
    auto {}
  }
}

resource "google_secret_manager_secret_version" "payments_api_key" {
  secret      = google_secret_manager_secret.payments_api_key.id
  secret_data = var.payments_api_key_secret
}
