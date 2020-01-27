export const html = `<div style="padding: 3rem"><div class="tab-box">
<div class="tab-box-options" role="tablist" aria-label="login-or-register">
  <button
    class="tab-box-option active"
    role="tab"
    aria-selected="true"
    aria-controls="login-panel"
    id="login-panel-tab"
    tabindex="0"
  >
    Login
  </button>
  <button
    class="tab-box-option"
    role="tab"
    aria-selected="false"
    aria-controls="register-panel"
    id="register-panel-tab"
    tabindex="-1"
  >
    Register
  </button>
</div>
<div
  class="tab-box-content"
  id="login-panel"
  role="tabpanel"
  tabindex="0"
  aria-labelledby="login-panel-tab"
>
  <form id="login-form">
    <h1>Login</h1>
    <input type="text" name="username" /><input
      type="password"
      name="password"
    /><button type="submit">Login</button>
  </form>
</div>
<div
  class="tab-box-content"
  id="register-panel"
  role="tabpanel"
  tabindex="0"
  aria-labelledby="register-panel-tab"
  hidden
>
  <form id="register-form">
    <h1>Register</h1>
    <input type="text" name="username" /><input
      type="password"
      name="password"
    /><button type="submit">Register</button>
  </form>
</div>
</div>
<span id='login-register-error-message' class='color-danger'></span>
</div>
`;
