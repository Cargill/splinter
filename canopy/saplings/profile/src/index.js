import React from 'react';
import ReactDOM from 'react-dom';
import './index.css';
import App from './App';
import { registerConfigSapling, registerApp } from "canopyjs";


// Comment this block for development
registerConfigSapling('profile', () => {
  if (window.location.pathname === '/profile') {
    registerApp(domNode => {
      console.log('Registering profile sapling');
      ReactDOM.render(<App />, domNode);
    });
  }
})

// Comment this line for production
// ReactDOM.render(<App />, document.getElementById('root'));
