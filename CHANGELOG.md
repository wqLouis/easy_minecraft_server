# Changelog

## 2026-05-16 — Auth rework: username-based authentication

### Breaking changes for frontend

#### 1. Registration: `email` → `username`

`POST /api/auth/register` now accepts `username` instead of `email`:

```json
// Old
{ "email": "user@example.com" }

// New
{ "username": "myuser" }
```

**Response** field `user.email` is now `user.username`.

#### 2. Authentication header format

**Old:** `Authorization: Bearer <64-char-api-key>`

**New:** `Authorization: Bearer <username>:<token>`

The token is the 64-char hex string returned during user creation.  
Both `username` and `token` are required — they are hashed together server-side.

```bash
# Old
curl -H "Authorization: Bearer ab12cd34..." ...

# New
curl -H "Authorization: Bearer admin:ab12cd34..." ...
```

#### 3. `GET /api/auth/me` response

```json
// Old
{ "user": { "email": "admin@example.com", ... } }

// New
{ "user": { "username": "admin", ... } }
```

#### 4. `GET /api/users` list

Field renamed from `"email"` → `"username"` in every user object.

#### 5. `PUT /api/users/{id}` update

Body field: `{ "email": "..." }` → `{ "username": "..." }`

#### 6. CLI user creation

```bash
# Old
backend create-sudo --email admin@example.com

# New
eazymc-backend create-sudo --username admin
```

Output now shows `Username` and `Token` separately, plus the exact header format.

#### 7. Database

The `users` table column `email` has been renamed to `username`.  
The stored hash is now `hash(token + username)` instead of `hash(token)`.

Existing databases must be reset: `eazymc-backend reset-db`
(⚠️ this deletes all users).

---

### Auth flow summary

| Concept | Description |
|---------|-------------|
| `username` | Human-readable label, stored lowercased. Not a secret. |
| `token` | 64-char hex string, generated server-side, shown once on user creation. |
| Credential hash | `argon2id(token + username)` stored in DB. Both values needed to verify. |
| Auth header | `Authorization: Bearer <username>:<token>` |
| Lookup | Server looks up by username, then verifies token. No more full-table scan. |
