function disabled() {
    return document.getElementById("update-shore").classList.toString().indexOf("disabled") != -1;
}

function disable_button(id) {
    var element = document.getElementById(id);
    if (element != null) {
        element.classList += " disabled";
        element.disabled = "disabled";
    }
}

function disable() {
    disable_button("resume-management");
    disable_button("pause-management");
    disable_button("update-shore");
    disable_button("set-timer");
    disable_button("override-we-turned-it-on");
}

function post(url) {
    const httpRequest = new XMLHttpRequest();
    httpRequest.onreadystatechange = () => {
        if (httpRequest.readyState === XMLHttpRequest.DONE) {
            window.location = window.location;
        }
    };
    httpRequest.open('POST', url, true);
    httpRequest.send();
}

window.pauseManagement = function() {
    if(!disabled()) {
        disable();
        var url = '/api/v1/do_not_run_generator/true';
        post(url);
    }
}

window.resumeManagement = function() {
    if(!disabled()) {
        disable();
        var url = '/api/v1/do_not_run_generator/false';
        post(url);
    }
}

window.updateShore = function() {
    if(!disabled()) {
        disable();
        var new_limit = document.getElementById("amps").value;
        var url = '/api/v1/shore_limit/' + new_limit;
        post(url);
    }
}

window.setTimer = function() {
    if(!disabled()) {
        disable();
        var time = document.getElementById("timer").value;
        var url = '/api/v1/timer/' + time;
        post(url);
    }
}

window.overrideWeTurnedItOn = function() {
    if(!disabled()) {
        disable();
        post('/api/v1/manage/true');
    }
}