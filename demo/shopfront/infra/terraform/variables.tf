variable "project" {
  type    = string
  default = "shopfront"
}

variable "region" {
  type    = string
  default = "europe-west3"
}

variable "db_tier" {
  type    = string
  default = "db-custom-1-3840"
}

variable "payments_api_key_secret" {
  type = string
}
