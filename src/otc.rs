// Infrastructure as a map: Terraform blocks per environment plus what the Kubernetes clusters run right now.
// Project path: `otc:<folder>`, where <folder> holds infrastructure-v2/ and the *-cce-*kubeconfig.yaml files.
// Cluster access is read-only (`kubectl get`).
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// One machine on the map.
pub struct Node {
    pub district: String,
    pub folder: String,
    pub title: String,
    pub body: String,
    /// 0 idle / scaled to zero, 1 degraded, 2 healthy; None for declared-only (Terraform) things
    pub state: Option<u8>,
    pub path: Option<String>,
    pub line: u32,
    pub items: Vec<(String, String)>,
    /// folder with the code this stands for, if one matches by name
    pub code: Option<String>,
}

pub fn parse(path: &str) -> Option<&str> {
    path.strip_prefix("otc:")
}

/// Short environment name: "dev", "prod", "global".
fn env_of(name: &str) -> String {
    if name.starts_with("dev") {
        "dev".into()
    } else if name.starts_with("prod") {
        "prod".into()
    } else {
        name.into()
    }
}

/// Folder for a Terraform block, by what it builds.
fn tf_folder(kind: &str) -> &'static str {
    let k = kind.to_lowercase();
    let has = |xs: &[&str]| xs.iter().any(|x| k.contains(x));
    if has(&["rds", "dds", "mongo", "postgres", "obs", "bucket"]) {
        "data"
    } else if has(&["apig", "gateway"]) {
        "gateway"
    } else if has(&["cce", "ecs", "node", "irsa", "image", "compute"]) {
        "compute"
    } else if has(&["vpc", "snat", "dns", "secgroup", "security", "elb", "loadbalancer", "eip", "vpn", "subnet", "network"]) {
        "network"
    } else {
        "other"
    }
}

/// Code folders a workload can stand for: `<repo>/services/<name>` and top-level repositories, by folder name.
fn code_dirs(root: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let dirs = |p: &Path| -> Vec<std::path::PathBuf> { std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default() };
    for repo in dirs(Path::new(root)) {
        out.entry(repo.file_name().unwrap().to_string_lossy().to_string()).or_insert(repo.to_string_lossy().to_string());
        for svc in dirs(&repo.join("services")) {
            // services win over repositories of the same name
            out.insert(svc.file_name().unwrap().to_string_lossy().to_string(), svc.to_string_lossy().to_string());
        }
    }
    out
}

/// The Kubernetes manifest under `code_dir` that defines workload `name`, preferring the one for `env`
/// ("prod" matches acme-production-v2, "dev" acme-development-v2).
pub fn manifest(code_dir: &str, name: &str, env: &str) -> Option<String> {
    fn walk(d: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().to_string();
            if p.is_dir() && n != "node_modules" && n != "target" && n != "dist" {
                walk(&p, out);
            } else if n.ends_with(".yaml") || n.ends_with(".yml") {
                out.push(p);
            }
        }
    }
    let mut files = vec![];
    walk(Path::new(code_dir), &mut files);
    files.sort();
    let named = regex::Regex::new(&format!(r#"(?m)^\s*name:\s*['"]?{}['"]?\s*$"#, regex::escape(name))).ok()?;
    let defines = |p: &std::path::PathBuf| std::fs::read_to_string(p).is_ok_and(|t| (t.contains("kind: Deployment") || t.contains("kind: StatefulSet")) && named.is_match(&t));
    let hits: Vec<_> = files.into_iter().filter(defines).collect();
    let env_word = match env {
        "prod" => "production",
        "dev" => "development",
        e => e,
    };
    hits.iter().find(|p| p.to_string_lossy().contains(env_word)).or(hits.first()).map(|p| p.to_string_lossy().to_string())
}

pub fn load(root: &str) -> (Vec<Node>, Vec<(usize, usize)>) {
    let code = code_dirs(root);
    let mut nodes: Vec<Node> = vec![];
    let mut links: Vec<(usize, usize)> = vec![];
    // ---- Terraform: every module/resource block of every environment ----
    let tf_root = Path::new(root).join("infrastructure-v2/cloud_infrastructure");
    let head = regex::Regex::new(r#"^(module|resource) "([\w-]+)"(?: "([\w-]+)")?\s*\{"#).unwrap();
    let source = regex::Regex::new(r#"source\s*=\s*"[^"]*/([\w-]+)""#).unwrap();
    let refs = regex::Regex::new(r"\bmodule\.([\w-]+)|\b(opentelekomcloud_\w+)\.([\w-]+)").unwrap();
    let mut envs: Vec<_> = std::fs::read_dir(&tf_root).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    envs.sort();
    let mut tf_bodies: Vec<(usize, String, String)> = vec![]; // (node, env, body)
    let mut tf_index: HashMap<(String, String), usize> = HashMap::new(); // (env, "module.x" | "type.name") -> node
    for env_dir in envs {
        let env = env_of(&env_dir.file_name().unwrap().to_string_lossy());
        let mut files: Vec<_> = std::fs::read_dir(&env_dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "tf")).collect()).unwrap_or_default();
        files.sort();
        for f in files {
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            let lines: Vec<&str> = text.lines().collect();
            for (i, l) in lines.iter().enumerate() {
                let Some(c) = head.captures(l) else { continue };
                let end = (i + 1..lines.len()).find(|&j| lines[j].starts_with('}')).unwrap_or(lines.len() - 1);
                let body = lines[i..=end].join("\n");
                let (key, kind, title) = if &c[1] == "module" {
                    let kind = source.captures(&body).map_or(c[2].to_string(), |s| s[1].to_string());
                    (format!("module.{}", &c[2]), kind.clone(), format!("{}  ({kind})", &c[2]))
                } else {
                    let ty = c[2].to_string();
                    let name = c.get(3).map_or("", |m| m.as_str()).to_string();
                    (format!("{ty}.{name}"), ty.clone(), format!("{} {name}", ty.trim_start_matches("opentelekomcloud_")))
                };
                tf_index.insert((env.clone(), key), nodes.len());
                tf_bodies.push((nodes.len(), env.clone(), body.clone()));
                nodes.push(Node {
                    district: env.clone(), folder: format!("terraform {}", tf_folder(&kind)), title,
                    body: body.lines().take(60).collect::<Vec<_>>().join("\n"), state: None,
                    path: Some(f.to_string_lossy().to_string()), line: i as u32 + 1, items: vec![], code: None,
                });
            }
        }
    }
    // a block that names another block of its environment depends on it
    for (n, env, body) in &tf_bodies {
        for c in refs.captures_iter(body) {
            let key = match (c.get(1), c.get(2), c.get(3)) {
                (Some(m), _, _) => format!("module.{}", m.as_str()),
                (_, Some(t), Some(name)) => format!("{}.{}", t.as_str(), name.as_str()),
                _ => continue,
            };
            if let Some(&t) = tf_index.get(&(env.clone(), key)) {
                if t != *n && !links.contains(&(*n, t)) {
                    links.push((*n, t));
                }
            }
        }
    }
    // ---- Kubernetes: what runs right now ----
    let mut kubeconfigs: Vec<_> = std::fs::read_dir(root).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with("kubeconfig.yaml")).collect()).unwrap_or_default();
    kubeconfigs.sort();
    for kc in kubeconfigs {
        let file = kc.file_name().unwrap().to_string_lossy().to_string();
        let env = env_of(file.split_once('-').map_or(file.as_str(), |x| x.1));
        let get = |what: &str| -> Value {
            Command::new("kubectl").args(["get", what, "-A", "-o", "json", "--request-timeout=20s"]).env("KUBECONFIG", &kc).output().ok()
                .and_then(|o| serde_json::from_slice(&o.stdout).ok()).unwrap_or(Value::Null)
        };
        let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let items = |v: &Value| v["items"].as_array().cloned().unwrap_or_default();
        let cluster = tf_index.get(&(env.clone(), "module.cce".into())).copied();
        // nodes
        for n in items(&get("nodes")) {
            let ready = n["status"]["conditions"].as_array().into_iter().flatten().any(|c| c["type"] == "Ready" && c["status"] == "True");
            let i = nodes.len();
            nodes.push(Node {
                district: env.clone(), folder: "k8s nodes".into(), title: s(&n["metadata"]["name"]),
                body: format!("{} cpu · {} memory · kubelet {}", s(&n["status"]["capacity"]["cpu"]), s(&n["status"]["capacity"]["memory"]), s(&n["status"]["nodeInfo"]["kubeletVersion"])),
                state: Some(if ready { 2 } else { 1 }), path: None, line: 0, items: vec![], code: None,
            });
            if let Some(c) = cluster {
                links.push((c, i));
            }
        }
        // workloads
        let mut by_name: HashMap<(String, String), usize> = HashMap::new(); // (namespace, name) -> node
        let mut env_refs: Vec<(usize, String, Vec<(String, String)>)> = vec![];
        for kind in ["deployments", "statefulsets"] {
            for d in items(&get(kind)) {
                let ns = s(&d["metadata"]["namespace"]);
                let name = s(&d["metadata"]["name"]);
                let want = d["spec"]["replicas"].as_u64().unwrap_or(1);
                let ready = d["status"]["readyReplicas"].as_u64().unwrap_or(0);
                let containers = d["spec"]["template"]["spec"]["containers"].as_array().cloned().unwrap_or_default();
                let image = containers.first().map(|c| s(&c["image"])).unwrap_or_default();
                let env_vars: Vec<(String, String)> = containers.iter().flat_map(|c| c["env"].as_array().cloned().unwrap_or_default()).map(|e| (s(&e["name"]), s(&e["value"]))).collect();
                let i = nodes.len();
                by_name.insert((ns.clone(), name.clone()), i);
                env_refs.push((i, ns.clone(), env_vars));
                nodes.push(Node {
                    code: code.get(name.strip_prefix("backend-v2-").unwrap_or(&name)).cloned(),
                    district: env.clone(), folder: format!("k8s {ns}"), title: name,
                    body: format!("{} {ready}/{want} ready\n{}", kind.trim_end_matches('s'), image.rsplit('/').next().unwrap_or(&image)),
                    state: Some(if want == 0 { 0 } else if ready < want { 1 } else { 2 }), path: None, line: 0, items: vec![],
                });
            }
        }
        // ingresses: host and path become crates of the workload they reach, which the ingress controller feeds
        let controller = by_name.iter().find(|((_, n), _)| n.contains("ingress") || n.contains("nginx")).map(|(_, &i)| i);
        for ing in items(&get("ingress")) {
            let ns = s(&ing["metadata"]["namespace"]);
            for rule in ing["spec"]["rules"].as_array().into_iter().flatten() {
                for p in rule["http"]["paths"].as_array().into_iter().flatten() {
                    let svc = s(&p["backend"]["service"]["name"]);
                    if let Some(&t) = by_name.get(&(ns.clone(), svc)) {
                        nodes[t].items.push((format!("{}{}", s(&rule["host"]), s(&p["path"])), "ingress".into()));
                        if let Some(c) = controller.filter(|&c| c != t && !links.contains(&(c, t))) {
                            links.push((c, t));
                        }
                    }
                }
            }
        }
        // service-to-service and service-to-database wiring from environment variables
        let pg = tf_index.get(&(env.clone(), "module.postgres_rds".into())).copied();
        let mongo = tf_index.iter().find(|((e, k), _)| *e == env && k.starts_with("module.mongodb")).map(|(_, &i)| i);
        for (i, ns, vars) in env_refs {
            for (name, value) in vars {
                let host = value.split(['.', ':']).next().unwrap_or("");
                let target = by_name.get(&(ns.clone(), host.to_string())).copied()
                    .or_else(|| if name.contains("POSTGRES") || name.starts_with("PG") || name.contains("DATABASE") { pg } else { None })
                    .or_else(|| if name.contains("MONGO") { mongo } else { None });
                if let Some(t) = target.filter(|&t| t != i && !links.contains(&(i, t))) {
                    links.push((i, t));
                }
            }
        }
    }
    (nodes, links)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // needs ~/Projects/Accounts/acme and cluster access
    fn acme_infrastructure_loads() {
        let (nodes, links) = super::load(&format!("{}/Projects/Accounts/acme", std::env::var("HOME").unwrap()));
        let mut folders: std::collections::BTreeMap<String, (usize, [usize; 4])> = Default::default();
        for n in &nodes {
            let e = folders.entry(format!("{}/{}", n.district, n.folder)).or_default();
            e.0 += 1;
            e.1[n.state.map_or(3, |s| s as usize)] += 1;
        }
        for (f, (n, st)) in &folders {
            println!("{f}: {n} (idle {} degraded {} healthy {} declared {})", st[0], st[1], st[2], st[3]);
        }
        let with_code = nodes.iter().filter(|n| n.code.is_some()).count();
        println!("{with_code} workloads matched to code, e.g. {:?}", nodes.iter().find_map(|n| n.code.as_ref().map(|c| (&n.title, c))));
        let station = super::manifest(&format!("{}/Projects/Accounts/acme/shopfront-api/services/station", std::env::var("HOME").unwrap()), "backend-v2-station", "prod");
        println!("station manifest: {station:?}");
        assert!(station.is_some_and(|m| m.contains("acme-production-v2")));
        let wired = links.iter().filter(|(a, b)| nodes[*a].folder.starts_with("k8s") && nodes[*b].folder.starts_with("k8s")).count();
        let to_db = links.iter().filter(|(a, b)| nodes[*a].folder.starts_with("k8s") && nodes[*b].folder == "terraform data").count();
        println!("{} links: {wired} between workloads, {to_db} workload -> database", links.len());
        assert!(folders.keys().any(|f| f.starts_with("prod/terraform")) && wired > 0);
    }
}
