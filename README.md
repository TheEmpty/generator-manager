# Generator Manager

This auto starts the generator when it's below "auto-start-soc" until it's "stop-charge-soc". Also adjusts shore or generator limit on the multiplus based on the generator status.

## Requirements:
* adjust config (see config.example.json)
* It can talk to LCI gateway on [http://192.168.1.4:8080/rest/things/](http://192.168.1.4:8080/rest/things/).
* You have a generator connected to your LCI system.
* You have a Victron MQTT endpoint with a multiplus

## _You might also like_

### [LCI Gateway Exporter](https://github.com/TheEmpty/lci-gateway-exporter)
Requires: LCI gateway available on [http://192.168.1.4:8080/rest/things/](http://192.168.1.4:8080/rest/things/).

Exports the current state of your connected devices. It is designed
to be used with [Prometheus](https://prometheus.io/) to capture
the state over time as metrics. And a tool like [Grafana](https://grafana.com/)
to visualize as charts, text, bars, etc.

## Kubernetes Example

```
apiVersion: apps/v1
kind: Deployment
metadata:
  name: generator-manager
spec:
  selector:
    matchLabels:
      app: generator-manager
  template:
    metadata:
      labels:
        app: generator-manager
    spec:
      restartPolicy: Always
      volumes:
        - name: config
          configMap:
            name: generator-manager-config
      containers:
        - name: generator-manager
          image: theempty/generator-manager
          resources: {}
          volumeMounts:
            - name: config
              mountPath: /etc/generator-manager
          args:
            - /etc/generator-manager/config.json

---

apiVersion: v1
kind: ConfigMap
metadata:
  name: generator-manager-config
data:
  config.json: |
    {
        "shore_limit": 15.0,
        "generator": {
            "limit": 45.5,
            "auto_start_soc": 30,
            "stop_charge_soc": 80
        },
        "mqtt": {
            "host": "192.168.50.5",
            "port": 1883,
            "user": "public",
            "password": "public"
        },
        "topics": {
            "current_limit": "{YOUR ID}/vebus/{VE BUS ID}/Ac/ActiveIn/CurrentLimit",
            "shore_connected": "{YOUR ID}/vebus/{VE BUS ID}/Ac/ActiveIn/Connected",
            "soc": "{YOUR ID}/battery/{BATTERY ID}/Soc"
        }
    }
```
