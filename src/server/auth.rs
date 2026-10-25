//! Role-Based Access Control (RBAC), user authentication, and credential storage.
//!
//! Enforces granular privileges across:
//! - System administration (`ServerAdmin`, `UserAdmin`, `ClusterAdmin`)
//! - Database & transaction operations (`DbAccess`, `TxManage`)
//! - Graph data read/write/schema mutations (`GraphRead`, `GraphWrite`, `GraphSchema`, `ExecuteAlgorithm`)

use super::crypto::{base64_decode, generate_bearer_token, hash_password, verify_password};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// =============================================================================
// Privileges & Permissions
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Privilege {
    /// Full server administrative operations (checkpoint, shutdown, full export/import)
    ServerAdmin,
    /// User account and role management (create user, change password, assign roles)
    UserAdmin,
    /// Inter-node cluster management (join cluster, failover inspection)
    ClusterAdmin,
    /// Connect to database and read cluster/server health & stats
    DbAccess,
    /// Start, commit, and rollback ACID transactions
    TxManage,
    /// Execute Cypher read queries (MATCH), vector search, and node/edge inspection
    GraphRead,
    /// Mutate graph topology (Cypher CREATE/SET/DELETE/MERGE, bulk import, delete)
    GraphWrite,
    /// Manage vector similarity indexes, fulltext indexes, and TTL rules
    GraphSchema,
    /// Execute Graph Data Science algorithms (PageRank, Shortest Path, Centrality)
    ExecuteAlgorithm,
}

// =============================================================================
// Roles
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub description: String,
    pub privileges: HashSet<Privilege>,
}

impl Role {
    pub fn new(name: &str, description: &str, privileges: HashSet<Privilege>) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            privileges,
        }
    }
}

// =============================================================================
// User Accounts & Sessions
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    pub username: String,
    pub password_hash: String,
    pub roles: Vec<String>,
    pub is_active: bool,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub roles: Vec<String>,
    pub is_active: bool,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionToken {
    pub token: String,
    pub username: String,
    pub expires_at: u64,
}

// =============================================================================
// Security Context (Evaluated per Request)
// =============================================================================

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub username: String,
    pub roles: Vec<String>,
    pub privileges: HashSet<Privilege>,
}

impl SecurityContext {
    /// Creates a system root context that possesses all privileges.
    pub fn root() -> Self {
        let mut privileges = HashSet::new();
        privileges.insert(Privilege::ServerAdmin);
        privileges.insert(Privilege::UserAdmin);
        privileges.insert(Privilege::ClusterAdmin);
        privileges.insert(Privilege::DbAccess);
        privileges.insert(Privilege::TxManage);
        privileges.insert(Privilege::GraphRead);
        privileges.insert(Privilege::GraphWrite);
        privileges.insert(Privilege::GraphSchema);
        privileges.insert(Privilege::ExecuteAlgorithm);

        Self {
            username: "root".to_string(),
            roles: vec!["admin".to_string()],
            privileges,
        }
    }

    /// Checks whether this security context contains the given privilege.
    /// Notice: `ServerAdmin` acts as superuser bypass for all privileges.
    pub fn has_privilege(&self, priv_req: &Privilege) -> bool {
        self.privileges.contains(&Privilege::ServerAdmin) || self.privileges.contains(priv_req)
    }

    /// Asserts that this security context has the required privilege.
    pub fn require(&self, priv_req: Privilege) -> Result<()> {
        if self.has_privilege(&priv_req) {
            Ok(())
        } else {
            Err(Error::PermissionDenied(format!(
                "Permission Denied: User '{}' with roles {:?} lacks required privilege '{:?}'",
                self.username, self.roles, priv_req
            )))
        }
    }
}

// =============================================================================
// Auth Persistent State DTO
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthStateDto {
    users: Vec<UserAccount>,
    custom_roles: Vec<Role>,
}

// =============================================================================
// AuthManager
// =============================================================================

pub struct AuthManager {
    auth_enabled: bool,
    storage_path: Option<PathBuf>,
    users: RwLock<HashMap<String, UserAccount>>,
    roles: RwLock<HashMap<String, Role>>,
    sessions: RwLock<HashMap<String, SessionToken>>,
    session_duration: Duration,
}

impl AuthManager {
    /// Creates a new AuthManager, optionally loading existing users from `data_dir/auth.json`.
    pub fn new(
        auth_enabled: bool,
        data_dir: Option<&Path>,
        default_admin_password: Option<&str>,
    ) -> Result<Self> {
        let mut default_roles = HashMap::new();

        // 1. admin: Superuser
        let mut admin_privs = HashSet::new();
        admin_privs.insert(Privilege::ServerAdmin);
        admin_privs.insert(Privilege::UserAdmin);
        admin_privs.insert(Privilege::ClusterAdmin);
        admin_privs.insert(Privilege::DbAccess);
        admin_privs.insert(Privilege::TxManage);
        admin_privs.insert(Privilege::GraphRead);
        admin_privs.insert(Privilege::GraphWrite);
        admin_privs.insert(Privilege::GraphSchema);
        admin_privs.insert(Privilege::ExecuteAlgorithm);
        default_roles.insert(
            "admin".to_string(),
            Role::new("admin", "Full cluster and database superuser", admin_privs),
        );

        // 2. architect: Schema, data, algorithms, transactions
        let mut arch_privs = HashSet::new();
        arch_privs.insert(Privilege::DbAccess);
        arch_privs.insert(Privilege::TxManage);
        arch_privs.insert(Privilege::GraphRead);
        arch_privs.insert(Privilege::GraphWrite);
        arch_privs.insert(Privilege::GraphSchema);
        arch_privs.insert(Privilege::ExecuteAlgorithm);
        default_roles.insert(
            "architect".to_string(),
            Role::new(
                "architect",
                "Data engineer with schema and algorithm rights",
                arch_privs,
            ),
        );

        // 3. editor: Read and write data & transactions
        let mut editor_privs = HashSet::new();
        editor_privs.insert(Privilege::DbAccess);
        editor_privs.insert(Privilege::TxManage);
        editor_privs.insert(Privilege::GraphRead);
        editor_privs.insert(Privilege::GraphWrite);
        default_roles.insert(
            "editor".to_string(),
            Role::new(
                "editor",
                "Application service with read and write privileges",
                editor_privs,
            ),
        );

        // 4. reader: Read-only access
        let mut reader_privs = HashSet::new();
        reader_privs.insert(Privilege::DbAccess);
        reader_privs.insert(Privilege::GraphRead);
        default_roles.insert(
            "reader".to_string(),
            Role::new(
                "reader",
                "Read-only analytics and inspection access",
                reader_privs,
            ),
        );

        let storage_path = data_dir.map(|d| d.join("auth.json"));
        let mut loaded_users = HashMap::new();

        if let Some(ref path) = storage_path {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(dto) = serde_json::from_str::<AuthStateDto>(&content) {
                        for u in dto.users {
                            loaded_users.insert(u.username.clone(), u);
                        }
                        for r in dto.custom_roles {
                            default_roles.insert(r.name.clone(), r);
                        }
                    }
                }
            }
        }

        // If no admin user exists, bootstrap default admin
        if !loaded_users.contains_key("admin") {
            let pass = default_admin_password.unwrap_or("admin");
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            loaded_users.insert(
                "admin".to_string(),
                UserAccount {
                    username: "admin".to_string(),
                    password_hash: hash_password(pass),
                    roles: vec!["admin".to_string()],
                    is_active: true,
                    created_at: now,
                },
            );
        }

        let manager = Self {
            auth_enabled,
            storage_path,
            users: RwLock::new(loaded_users),
            roles: RwLock::new(default_roles),
            sessions: RwLock::new(HashMap::new()),
            session_duration: Duration::from_secs(24 * 3600), // 24 hours
        };

        // Persist initial state if we have a path
        if manager.storage_path.is_some() {
            let _ = manager.save_to_disk();
        }

        Ok(manager)
    }

    /// Whether authentication is enabled.
    pub fn is_auth_enabled(&self) -> bool {
        self.auth_enabled
    }

    /// Authenticates username and password, returning a new SessionToken if valid.
    pub fn login(&self, username: &str, password: &str) -> Result<SessionToken> {
        let users = self
            .users
            .read()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        let user = users
            .get(username)
            .ok_or_else(|| Error::Authentication("Invalid username or password".to_string()))?;

        if !user.is_active {
            return Err(Error::Authentication(
                "User account is disabled".to_string(),
            ));
        }

        if !verify_password(password, &user.password_hash) {
            return Err(Error::Authentication(
                "Invalid username or password".to_string(),
            ));
        }

        let token_str = generate_bearer_token();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = now + self.session_duration.as_secs();

        let session = SessionToken {
            token: token_str.clone(),
            username: username.to_string(),
            expires_at,
        };

        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        sessions.insert(token_str, session.clone());

        Ok(session)
    }

    /// Validates an incoming HTTP Authorization header string (`Bearer <token>` or `Basic <base64>`).
    pub fn authenticate_request(&self, auth_header: Option<&str>) -> Result<SecurityContext> {
        if !self.auth_enabled {
            return Ok(SecurityContext::root());
        }

        let header = match auth_header {
            Some(h) if !h.trim().is_empty() => h.trim(),
            _ => {
                return Err(Error::Authentication(
                    "Missing Authorization header".to_string(),
                ));
            }
        };

        if let Some(token) = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
        {
            let token = token.trim();
            let sessions = self
                .sessions
                .read()
                .map_err(|_| Error::Storage("Lock poisoned".into()))?;
            let session = sessions.get(token).ok_or_else(|| {
                Error::Authentication("Invalid or expired session token".to_string())
            })?;

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if session.expires_at < now {
                return Err(Error::Authentication("Session token expired".to_string()));
            }

            self.build_security_context(&session.username)
        } else if let Some(basic) = header
            .strip_prefix("Basic ")
            .or_else(|| header.strip_prefix("basic "))
        {
            let basic = basic.trim();
            let decoded_bytes = base64_decode(basic)
                .ok_or_else(|| Error::Authentication("Invalid Base64 in Basic Auth".to_string()))?;
            let decoded_str = String::from_utf8(decoded_bytes)
                .map_err(|_| Error::Authentication("Invalid UTF-8 in Basic Auth".to_string()))?;

            let mut parts = decoded_str.splitn(2, ':');
            let username = parts.next().unwrap_or("");
            let password = parts.next().unwrap_or("");

            let users = self
                .users
                .read()
                .map_err(|_| Error::Storage("Lock poisoned".into()))?;
            let user = users
                .get(username)
                .ok_or_else(|| Error::Authentication("Invalid username or password".to_string()))?;

            if !user.is_active {
                return Err(Error::Authentication(
                    "User account is disabled".to_string(),
                ));
            }

            if !verify_password(password, &user.password_hash) {
                return Err(Error::Authentication(
                    "Invalid username or password".to_string(),
                ));
            }

            self.build_security_context(username)
        } else {
            Err(Error::Authentication(
                "Unsupported Authorization header format (expected Bearer or Basic)".to_string(),
            ))
        }
    }

    /// Constructs the merged `SecurityContext` with all accumulated privileges across assigned roles.
    pub fn build_security_context(&self, username: &str) -> Result<SecurityContext> {
        let users = self
            .users
            .read()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        let user = users
            .get(username)
            .ok_or_else(|| Error::Authentication(format!("User '{}' not found", username)))?;

        let roles_map = self
            .roles
            .read()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        let mut privileges = HashSet::new();

        for role_name in &user.roles {
            if let Some(role) = roles_map.get(role_name) {
                for priv_item in &role.privileges {
                    privileges.insert(priv_item.clone());
                }
            }
        }

        Ok(SecurityContext {
            username: user.username.clone(),
            roles: user.roles.clone(),
            privileges,
        })
    }

    /// Creates a new user account. Requires `UserAdmin`.
    pub fn create_user(&self, username: &str, password: &str, roles: Vec<String>) -> Result<()> {
        let trimmed_user = username.trim();
        if trimmed_user.is_empty() {
            return Err(Error::InvalidOperation(
                "Username cannot be empty".to_string(),
            ));
        }
        if password.is_empty() {
            return Err(Error::InvalidOperation(
                "Password cannot be empty".to_string(),
            ));
        }

        let mut users = self
            .users
            .write()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        if users.contains_key(trimmed_user) {
            return Err(Error::InvalidOperation(format!(
                "User '{}' already exists",
                trimmed_user
            )));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let account = UserAccount {
            username: trimmed_user.to_string(),
            password_hash: hash_password(password),
            roles,
            is_active: true,
            created_at: now,
        };

        users.insert(trimmed_user.to_string(), account);
        drop(users);

        self.save_to_disk()
    }

    /// Updates a user's password.
    pub fn update_password(&self, username: &str, new_password: &str) -> Result<()> {
        if new_password.is_empty() {
            return Err(Error::InvalidOperation(
                "New password cannot be empty".to_string(),
            ));
        }

        let mut users = self
            .users
            .write()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        let user = users
            .get_mut(username)
            .ok_or_else(|| Error::InvalidOperation(format!("User '{}' not found", username)))?;

        user.password_hash = hash_password(new_password);
        drop(users);

        self.save_to_disk()
    }

    /// Assigns roles to an existing user.
    pub fn assign_roles(&self, username: &str, roles: Vec<String>) -> Result<()> {
        let mut users = self
            .users
            .write()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        let user = users
            .get_mut(username)
            .ok_or_else(|| Error::InvalidOperation(format!("User '{}' not found", username)))?;

        user.roles = roles;
        drop(users);

        self.save_to_disk()
    }

    /// Deletes a user account.
    pub fn delete_user(&self, username: &str) -> Result<()> {
        if username == "admin" {
            return Err(Error::InvalidOperation(
                "Cannot delete the root 'admin' user".to_string(),
            ));
        }

        let mut users = self
            .users
            .write()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        if users.remove(username).is_none() {
            return Err(Error::InvalidOperation(format!(
                "User '{}' not found",
                username
            )));
        }
        drop(users);

        self.save_to_disk()
    }

    /// Returns a list of all registered users.
    pub fn list_users(&self) -> Result<Vec<UserInfo>> {
        let users = self
            .users
            .read()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        let mut list: Vec<UserInfo> = users
            .values()
            .map(|u| UserInfo {
                username: u.username.clone(),
                roles: u.roles.clone(),
                is_active: u.is_active,
                created_at: u.created_at,
            })
            .collect();
        list.sort_by(|a, b| a.username.cmp(&b.username));
        Ok(list)
    }

    /// Atomically persists user accounts and roles to disk if configured.
    fn save_to_disk(&self) -> Result<()> {
        let path = match self.storage_path {
            Some(ref p) => p,
            None => return Ok(()),
        };

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let users = self
            .users
            .read()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;
        let roles = self
            .roles
            .read()
            .map_err(|_| Error::Storage("Lock poisoned".into()))?;

        let user_list: Vec<UserAccount> = users.values().cloned().collect();
        let custom_roles: Vec<Role> = roles
            .values()
            .filter(|r| !["admin", "architect", "editor", "reader"].contains(&r.name.as_str()))
            .cloned()
            .collect();

        let dto = AuthStateDto {
            users: user_list,
            custom_roles,
        };

        let json = serde_json::to_string_pretty(&dto)
            .map_err(|e| Error::Storage(format!("Failed to serialize auth data: {}", e)))?;

        let tmp_path = path.with_extension("tmp");
        let mut file = File::create(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);

        fs::rename(tmp_path, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_login_and_security_context() {
        let auth = AuthManager::new(true, None, Some("admin123")).unwrap();

        // 1. Invalid password
        assert!(auth.login("admin", "wrong").is_err());

        // 2. Valid password
        let session = auth
            .login("admin", "admin123")
            .expect("Admin login succeeds");
        assert_eq!(session.username, "admin");

        // 3. Bearer authentication
        let auth_hdr = format!("Bearer {}", session.token);
        let ctx = auth
            .authenticate_request(Some(&auth_hdr))
            .expect("Bearer auth succeeds");
        assert_eq!(ctx.username, "admin");
        assert!(ctx.has_privilege(&Privilege::ServerAdmin));
        assert!(ctx.has_privilege(&Privilege::GraphWrite));

        // 4. Basic Auth
        let basic_val = crate::server::crypto::base64_encode(b"admin:admin123");
        let basic_hdr = format!("Basic {}", basic_val);
        let ctx_basic = auth
            .authenticate_request(Some(&basic_hdr))
            .expect("Basic auth succeeds");
        assert_eq!(ctx_basic.username, "admin");
    }

    #[test]
    fn test_rbac_roles_privilege_enforcement() {
        let auth = AuthManager::new(true, None, Some("admin123")).unwrap();

        // Create a 'reader' user
        auth.create_user("alice", "alice123", vec!["reader".to_string()])
            .unwrap();

        let session = auth.login("alice", "alice123").unwrap();
        let ctx = auth
            .authenticate_request(Some(&format!("Bearer {}", session.token)))
            .unwrap();

        // Alice should have GraphRead, but NOT GraphWrite or ServerAdmin
        assert!(ctx.has_privilege(&Privilege::GraphRead));
        assert!(!ctx.has_privilege(&Privilege::GraphWrite));
        assert!(!ctx.has_privilege(&Privilege::ServerAdmin));

        assert!(ctx.require(Privilege::GraphRead).is_ok());
        assert!(ctx.require(Privilege::GraphWrite).is_err());
    }
}
