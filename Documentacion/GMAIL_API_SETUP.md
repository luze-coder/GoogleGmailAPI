# Configuracion de Gmail API

Esta configuracion usa OAuth 2.0 para que el worker acceda a una cuenta Gmail. No se utiliza una cuenta de servicio. El cliente creado es de tipo **Aplicacion web**, con sus propias credenciales y permisos OAuth.

## 1. Crear el proyecto y habilitar Gmail API

1. Abre [Google Cloud Console](https://console.cloud.google.com/) y crea un proyecto.
2. Selecciona ese proyecto desde el selector superior.
3. Ve a **APIs y servicios > Biblioteca**.
4. Busca **Gmail API** y presiona **Habilitar**.

## 2. Configurar la pantalla de consentimiento OAuth

1. Ve a **APIs y servicios > Pantalla de consentimiento OAuth**.
2. Selecciona el tipo de usuario correspondiente. Para una cuenta personal, normalmente es **Externo**.
3. Completa el nombre de la aplicacion, el correo de soporte y el correo de contacto del desarrollador.
4. Agrega el permiso:
	```text
	https://www.googleapis.com/auth/gmail.readonly
	```
5. Si la aplicacion esta en estado **Testing**, abre la seccion **Usuarios de prueba**.
6. Presiona **Add users** y agrega la cuenta Gmail que vas a consultar, con la cual en pasos posteriores te vas a logear en OAuth 2.0 Playground, para que pueda tener los permisos necesarios.

Este paso es obligatorio en modo de prueba. Si la cuenta no esta agregada como usuario de prueba, Google puede impedir la autorizacion aunque el cliente y el scope sean correctos.

## 3. Crear el cliente OAuth web

1. Ve a **APIs y servicios > Credenciales**.
2. Presiona **Crear credenciales > ID de cliente OAuth**.
3. Selecciona **Aplicacion web**.
4. En **URI de redireccionamiento autorizados**, agrega exactamente:
	```text
	https://developers.google.com/oauthplayground
	```
5. Crea el cliente y copia su `Client ID` y `Client secret`.

Estas credenciales pertenecen al cliente web creado en este proyecto. El `refresh_token` que vamos a obtener en OAuth 2.0 Playground debe generarse usando exactamente este mismo cliente.

## 4. Autorizar con OAuth 2.0 Playground

Abre [OAuth 2.0 Playground](https://developers.google.com/oauthplayground/).

### Configuracion propia

1. Presiona el icono de configuracion.
2. Activa **Use your own OAuth credentials**.
3. Ingresa el `Client ID` y `Client secret` del cliente **Aplicacion web** creado en el paso anterior.
4. Usa esta configuracion OAuth:
	- **OAuth flow:** `Server-side`
	- **Authorization endpoint:** `https://accounts.google.com/o/oauth2/v2/auth`
	- **Token endpoint:** `https://oauth2.googleapis.com/token`
	- **Access token location:** `Authorization header with Bearer prefix`
	- **Access type:** `Offline`
	- **Force prompt:** activado al generar el token

### Step 1: autorizar el scope

1. En **Step 1**, agrega o selecciona:
	```text
	https://www.googleapis.com/auth/gmail.readonly
	```
2. Presiona **Authorize APIs**.
3. Inicia sesion con la cuenta que agregaste como usuario de prueba.
4. Acepta los permisos solicitados.

### Step 2: obtener los tokens

1. Presiona **Exchange authorization code for tokens**.
2. Copia el `refresh_token` que devuelve Playground.

### Step 3: probar Gmail API

Despues del Step 2, en **Step 3** completa:

- **HTTP method:** `GET`
- **Request URI:**
  ```text
  https://gmail.googleapis.com/gmail/v1/users/me/messages
  ```
- **Authorization:** deja que Playground agregue automaticamente el token Bearer.

Opcionalmente agrega estos parametros:

```text
maxResults=5
q=has:attachment
```

Presiona **Send the request**. Una respuesta JSON con campos como `messages`, `nextPageToken` o `resultSizeEstimate` confirma que la autorizacion y Gmail API funcionan.

No publiques el refresh token ni el client secret.

## 5. Configurar el worker

Desde la raiz del proyecto, crea `.env` a partir de `.env.example` y completa los tres valores:

```text
GMAIL_CLIENT_ID=...
GMAIL_CLIENT_SECRET=...
GMAIL_REFRESH_TOKEN=...
```

El archivo `.env` esta ignorado por Git. Al iniciar, el worker lo carga automaticamente.

El worker envia automaticamente este request para renovar el access token:

```text
POST https://oauth2.googleapis.com/token
Content-Type: application/x-www-form-urlencoded

client_id=...
client_secret=...
refresh_token=...
grant_type=refresh_token
```

## 6. Ejecutar una captura

```powershell
cargo run -- --init-db
cargo run -- --queue-example
cargo run -- --process-one
cargo run -- --list-tasks
```

El ultimo comando muestra el estado de la tarea. Puedes inspeccionar los eventos con cualquier cliente SQLite:

```sql
SELECT task_id, event_type, details, created_at
FROM worker_events
ORDER BY id;
```