let shareform = document.getElementById('shareform');
let sharefieldset = document.getElementById('sharefieldset');
let layerselector = document.getElementById('layer');
let shareinput = document.getElementById('share');
let formresponse = document.getElementById('formresponse');
let layerstatus = document.getElementById('layerstatus');

function formMessage(message) {
        formresponse.className = '';
        formresponse.textContent = message;
}

function formError(err) {
        formresponse.className = 'red';
        formresponse.textContent = `Error: ${err}`;
}

function formSubmitHandler(e) {
    e.preventDefault();

    let uuid = layerselector.value;
    let share = shareinput.value;

    if (!uuid || !share) {
        formError('Please select a layer and enter a share.');
        return;
    }

    sharefieldset.setAttribute('disabled', 'disabled');

    shareinput.value = '';

    fetch(`/layer/${uuid}/share`, {
        method: 'POST',
        body: share,
    }).then(res => {
        res.text().then(text => {
            if (res.ok) {
                formMessage(text);
                requestLayerStatus().catch(formError);
            } else {
                    formError(`${res.statusText} (${text})`);
            }
        });
    }).catch(formError).finally(() => {
        sharefieldset.removeAttribute('disabled');
    })
}

async function requestLayerStatus() {
    return fetch('/layers').then(res => res.json()).then(layers => {
        layerselector.options.length = 0;
        layerstatus.innerHTML = '';

        if (layers.length === 0) {
            layerstatus.innerHTML = 'No layers found.';
            return;
        }

        layers.forEach(layer => {
            if (layer.state == 'idle') {
                layerselector.appendChild(new Option(layer.uuid));
            }
            let li = document.createElement('li');
            let shares_needed = 1;
            if (layer.state != 'idle') {
                shares_needed = ' no more';
            } else if (layer.vsss != null) {
                shares_needed = layer.vsss.threshold;
            }
            li.innerHTML = `<b>${layer.uuid}</b>: ${layer.state}, ${shares_needed} share(s) required`;
            layerstatus.appendChild(li);
        })
    })
}

async function liveRequestLayerStatus() {
    await requestLayerStatus().catch(formError).finally(() => {
        setTimeout(liveRequestLayerStatus, 10000);
    });
}

liveRequestLayerStatus().then(() => {
   sharefieldset.removeAttribute('disabled'); 
   shareform.addEventListener('submit', formSubmitHandler);
});