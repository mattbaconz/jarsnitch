package fixture;

import java.io.ObjectInputStream;
import java.io.Serializable;

// A serialization hook reading its own fields back: not reported.
final class Data implements Serializable {
    private Object value;

    private void readObject(ObjectInputStream in) throws Exception {
        in.defaultReadObject();
        value = in.readObject();
    }
}
