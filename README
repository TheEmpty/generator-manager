# Generator Manager

This auto starts the generator when it's below "auto-start-soc" until it's "stop-charge-soc". Also adjusts shore or generator limit on the multiplus based on the generator status.

## Requirements:
* adjust config
* It can talk to LCI gateway on [http://192.168.1.4:8080/rest/things/](http://192.168.1.4:8080/rest/things/).
* You have a Victron MQTT with multiplus

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
            "auto-start-soc": 30,
            "stop-charge-soc": 80
        },
        "mqtt": {
            "host": "192.168.50.5",
            "port": 1883,
            "user": "public",
            "password": "public"
        },
        "topics": {
            "curret_limit": "{YOUR ID}/vebus/{VE BUS ID}/Ac/ActiveIn/CurrentLimit",
            "soc": "+/battery/+/Soc"
        },
        "prowl_api_keys": [
            "REPLACE WITH YOURS"
        ]
    }
```
